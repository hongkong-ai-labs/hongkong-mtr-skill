use async_trait::async_trait;
use serde::de::DeserializeOwned;
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum HttpError {
    #[error("MTR upstream returned HTTP {status}{body}")]
    Status {
        status: u16,
        body: String,
        url: String,
    },
    #[error("Could not reach MTR upstream ({url}): {detail}")]
    Request { url: String, detail: String },
    #[error("MTR upstream returned invalid JSON ({url}): {source}")]
    Json {
        url: String,
        #[source]
        source: serde_json::Error,
    },
}

impl HttpError {
    pub fn retriable(&self) -> bool {
        match self {
            Self::Status { status, .. } => *status == 429 || *status >= 500,
            Self::Request { .. } => true,
            Self::Json { .. } => false,
        }
    }
}

/// HTTP operations needed by the data loaders. Native builds use reqwest;
/// Cloudflare Workers use the Workers `fetch` API.
#[async_trait]
pub trait HttpBackend: Clone + Send + Sync + 'static {
    async fn get_text(&self, url: &str, headers: &[(&str, &str)]) -> Result<String, HttpError>;

    async fn post_text(
        &self,
        url: &str,
        body: &Value,
        headers: &[(&str, &str)],
    ) -> Result<String, HttpError>;

    async fn get_json<T: DeserializeOwned + Send>(
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

    async fn post_json<T: DeserializeOwned + Send>(
        &self,
        url: &str,
        body: &Value,
        headers: &[(&str, &str)],
    ) -> Result<T, HttpError> {
        let text = self.post_text(url, body, headers).await?;
        serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|source| {
            HttpError::Json {
                url: url.to_string(),
                source,
            }
        })
    }
}

#[cfg(feature = "native")]
#[derive(Clone)]
pub struct HttpClient {
    client: reqwest::Client,
    user_agent: String,
}

#[cfg(feature = "native")]
impl HttpClient {
    pub fn new(timeout: std::time::Duration, user_agent: String) -> anyhow::Result<Self> {
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
        body: Option<&Value>,
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
            if let Some(body) = body {
                request = request.json(body);
            }

            let result = match request.send().await {
                Ok(response) => {
                    let status = response.status();
                    if status.is_success() {
                        response.text().await.map_err(|source| HttpError::Request {
                            url: url.to_string(),
                            detail: source.to_string(),
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
                    detail: source.to_string(),
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
                    tokio::time::sleep(std::time::Duration::from_millis(
                        250 * (attempt as u64 + 1),
                    ))
                    .await;
                }
            }
        }

        Err(last_error.expect("at least one request attempt"))
    }
}

#[cfg(feature = "native")]
#[async_trait]
impl HttpBackend for HttpClient {
    async fn get_text(&self, url: &str, headers: &[(&str, &str)]) -> Result<String, HttpError> {
        self.request_text(reqwest::Method::GET, url, headers, None)
            .await
    }

    async fn post_text(
        &self,
        url: &str,
        body: &Value,
        headers: &[(&str, &str)],
    ) -> Result<String, HttpError> {
        self.request_text(reqwest::Method::POST, url, headers, Some(body))
            .await
    }
}
