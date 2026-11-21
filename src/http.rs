use std::time::Duration;

use reqwest::StatusCode;
use serde::Serialize;
use serde::de::DeserializeOwned;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum HttpError {
    #[error("MTR upstream returned HTTP {status}{body}")]
    Status {
        status: u16,
        body: String,
        url: String,
    },
    #[error("Could not reach MTR upstream ({url}): {source}")]
    Request {
        url: String,
        #[source]
        source: reqwest::Error,
    },
    #[error("MTR upstream returned invalid JSON ({url}): {source}")]
    Json {
        url: String,
        #[source]
        source: serde_json::Error,
    },
}

impl HttpError {
    fn retriable(&self) -> bool {
        match self {
            Self::Status { status, .. } => *status == 429 || *status >= 500,
            Self::Request { .. } => true,
            Self::Json { .. } => false,
        }
    }
}

#[derive(Clone)]
pub struct HttpClient {
    client: reqwest::Client,
    user_agent: String,
}

impl HttpClient {
    pub fn new(timeout: Duration, user_agent: String) -> anyhow::Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .user_agent(&user_agent)
            .build()?;
        Ok(Self { client, user_agent })
    }

    async fn request_text(
        &self,
        method: reqwest::Method,
        url: &str,
        headers: &[(&str, &str)],
        body: Option<serde_json::Value>,
    ) -> Result<String, HttpError> {
        let mut last_error = None;

        for attempt in 0..=1 {
            let mut request = self
                .client
                .request(method.clone(), url)
                .header("User-Agent", &self.user_agent)
                .header("Accept", "application/json, text/xml, text/csv, */*");
            for (name, value) in headers {
                request = request.header(*name, *value);
            }
            if let Some(body) = &body {
                request = request.json(body);
            }

            let result = match request.send().await {
                Ok(response) => {
                    let status = response.status();
                    if status.is_success() {
                        response.text().await.map_err(|source| HttpError::Request {
                            url: url.to_string(),
                            source,
                        })
                    } else {
                        let snippet = response
                            .text()
                            .await
                            .unwrap_or_default()
                            .split_whitespace()
                            .collect::<Vec<_>>()
                            .join(" ");
                        let snippet = snippet.chars().take(300).collect::<String>();
                        Err(HttpError::Status {
                            status: status.as_u16(),
                            body: if snippet.is_empty() {
                                String::new()
                            } else {
                                format!(": {snippet}")
                            },
                            url: url.to_string(),
                        })
                    }
                }
                Err(source) => Err(HttpError::Request {
                    url: url.to_string(),
                    source,
                }),
            };

            match result {
                Ok(text) => return Ok(text),
                Err(error) => {
                    let retriable = error.retriable();
                    last_error = Some(error);
                    if !retriable || attempt == 1 {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(250 * (attempt as u64 + 1))).await;
                }
            }
        }

        Err(last_error.expect("at least one request attempt"))
    }

    pub async fn get_text(&self, url: &str, headers: &[(&str, &str)]) -> Result<String, HttpError> {
        self.request_text(reqwest::Method::GET, url, headers, None)
            .await
    }

    pub async fn get_json<T: DeserializeOwned>(
        &self,
        url: &str,
        headers: &[(&str, &str)],
    ) -> Result<T, HttpError> {
        let text = self.get_text(url, headers).await?;
        serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|source| {
            HttpError::Json {
                url: url.to_string(),
                source,
            }
        })
    }

    pub async fn post_json<B: Serialize, T: DeserializeOwned>(
        &self,
        url: &str,
        body: &B,
        headers: &[(&str, &str)],
    ) -> Result<T, HttpError> {
        let value = serde_json::to_value(body).map_err(|source| HttpError::Json {
            url: url.to_string(),
            source,
        })?;
        let text = self
            .request_text(reqwest::Method::POST, url, headers, Some(value))
            .await?;
        serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|source| {
            HttpError::Json {
                url: url.to_string(),
                source,
            }
        })
    }

    pub fn status_is_success(status: StatusCode) -> bool {
        status.is_success()
    }
}
