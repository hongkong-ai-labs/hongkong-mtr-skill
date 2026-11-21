use anyhow::{Result, anyhow};
use chrono::{SecondsFormat, Utc};
use serde::Deserialize;

use crate::app::App;
use crate::config::Config;
use crate::constants::line_name_for_code;
use crate::http::HttpBackend;
use crate::model::{Alert, AlertCounts, AlertSeverity, AlertsResult};
use crate::text::{html_to_text, one_line};

const LATE_CERT_BASE: &str = "https://tnews.mtr.com.hk/alert/";

#[derive(Debug, Clone, Deserialize)]
pub struct RawAlert {
    #[serde(rename = "newsType", default)]
    pub news_type: String,
    #[serde(rename = "tsiType", default)]
    pub tsi_type: String,
    #[serde(default)]
    pub status: i64,
    #[serde(default)]
    pub id: String,
    #[serde(rename = "lineCodes", default)]
    pub line_codes: Option<Vec<String>>,
    #[serde(rename = "alertTitle", default)]
    pub alert_title: String,
    #[serde(rename = "alertTitleTc", default)]
    pub alert_title_tc: String,
    #[serde(rename = "alertContent", default)]
    pub alert_content: String,
    #[serde(rename = "alertContentTc", default)]
    pub alert_content_tc: String,
    #[serde(default)]
    pub cause: String,
    #[serde(rename = "causeTc", default)]
    pub cause_tc: String,
    #[serde(rename = "show_alttrans", default)]
    pub show_alttrans: i64,
    #[serde(rename = "show_shuttlebus", default)]
    pub show_shuttlebus: i64,
    #[serde(rename = "show_map", default)]
    pub show_map: i64,
    #[serde(rename = "issuedTime", default)]
    pub issued_time: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct RawAlertResponse {
    #[serde(default)]
    data: Vec<RawAlert>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawLateCert {
    #[serde(default)]
    pub id: String,
    #[serde(rename = "lineCodes", default)]
    pub line_codes: Vec<String>,
    #[serde(default)]
    pub title: String,
    #[serde(rename = "titleTc", default)]
    pub title_tc: String,
    #[serde(rename = "issuedTime", default)]
    pub issued_time: String,
    #[serde(default)]
    pub url: String,
    #[serde(rename = "url_tc", default)]
    pub url_tc: String,
}

#[derive(Debug, Clone, Deserialize)]
struct RawLateCertResponse {
    #[serde(default)]
    latecerts: Vec<RawLateCert>,
}

#[derive(Debug, Clone)]
pub struct AlertsRaw {
    pub alerts: Vec<RawAlert>,
    pub late_certs: Vec<RawLateCert>,
}

pub async fn load_raw_alerts(http: &impl HttpBackend, config: &Config) -> Result<AlertsRaw> {
    let (alerts, late_certs) = futures::join!(
        http.get_json::<RawAlertResponse>(&config.endpoints.alerts, &[]),
        http.get_json::<RawLateCertResponse>(&config.endpoints.late_cert, &[]),
    );

    if alerts.is_err() && late_certs.is_err() {
        return Err(anyhow!(
            "{}",
            alerts
                .err()
                .map(|error| error.to_string())
                .unwrap_or_default()
        ));
    }

    Ok(AlertsRaw {
        alerts: alerts.map(|value| value.data).unwrap_or_default(),
        late_certs: late_certs.map(|value| value.latecerts).unwrap_or_default(),
    })
}

fn late_cert_url(id: &str, lang: &str) -> Option<String> {
    let numeric = id.strip_prefix("latecert-").unwrap_or(id);
    if numeric.is_empty() {
        return None;
    }
    Some(format!("{LATE_CERT_BASE}latecert/{numeric}_{lang}.pdf"))
}

pub fn classify_alert_severity(
    kind: &str,
    title: &str,
    show_map: bool,
    show_shuttle_bus: bool,
    show_alternative_transport: bool,
) -> AlertSeverity {
    if kind == "LateCert" {
        return AlertSeverity::Record;
    }
    if show_map || show_shuttle_bus || show_alternative_transport {
        return AlertSeverity::Disruption;
    }
    let lower = title.to_lowercase();
    if lower.contains("disruption") || lower.contains("suspend") {
        return AlertSeverity::Disruption;
    }
    if lower.contains("delay")
        || lower.contains("additional waiting")
        || lower.contains("additional travelling")
    {
        return AlertSeverity::Delay;
    }
    AlertSeverity::Notice
}

pub struct AlertOptions<'a> {
    pub line: Option<&'a str>,
    pub include_content: bool,
    pub lang: &'a str,
    pub active_only: bool,
}

impl Default for AlertOptions<'_> {
    fn default() -> Self {
        Self {
            line: None,
            include_content: true,
            lang: "en",
            active_only: false,
        }
    }
}

pub async fn get_alerts(
    app: &App<impl HttpBackend>,
    options: AlertOptions<'_>,
) -> Result<AlertsResult> {
    let raw = app.alerts().await?;
    let lang = if options.lang == "tc" { "tc" } else { "en" };
    let line = options.line.map(str::to_ascii_uppercase);

    let mut alerts: Vec<Alert> = raw
        .alerts
        .iter()
        .map(|alert| {
            let numeric_id = alert.id.strip_prefix("latecert-").unwrap_or(&alert.id);
            let late_cert = raw
                .late_certs
                .iter()
                .find(|cert| cert.id == numeric_id || alert.id.contains(&cert.id));
            let title = if lang == "tc" {
                &alert.alert_title_tc
            } else {
                &alert.alert_title
            };
            let content_html = if lang == "tc" {
                &alert.alert_content_tc
            } else {
                &alert.alert_content
            };
            let lines = alert.line_codes.clone().unwrap_or_default();
            let severity = classify_alert_severity(
                &alert.news_type,
                &alert.alert_title,
                alert.show_map == 1,
                alert.show_shuttlebus == 1,
                alert.show_alttrans == 1,
            );
            let details_url = match late_cert {
                Some(cert) if !cert.url.is_empty() => Some(format!(
                    "{LATE_CERT_BASE}{}",
                    cert.url.trim_start_matches('/')
                )),
                _ if alert.news_type == "LateCert" => late_cert_url(&alert.id, lang),
                _ => None,
            };

            Alert {
                id: alert.id.clone(),
                kind: alert.news_type.clone(),
                tsi_type: alert.tsi_type.clone(),
                active: alert.news_type != "LateCert" && alert.status == 0,
                severity,
                line_names: lines.iter().map(|code| line_name_for_code(code)).collect(),
                lines,
                title: one_line(Some(if !title.is_empty() {
                    title
                } else {
                    late_cert.map(|cert| cert.title.as_str()).unwrap_or("")
                })),
                title_tc: one_line(Some(if !alert.alert_title_tc.is_empty() {
                    &alert.alert_title_tc
                } else {
                    late_cert.map(|cert| cert.title_tc.as_str()).unwrap_or("")
                })),
                content: if options.include_content {
                    html_to_text(Some(content_html))
                } else {
                    String::new()
                },
                content_tc: if options.include_content && lang == "tc" {
                    html_to_text(Some(&alert.alert_content_tc))
                } else {
                    String::new()
                },
                cause: html_to_text(Some(if lang == "tc" {
                    &alert.cause_tc
                } else {
                    &alert.cause
                })),
                cause_tc: if lang == "tc" {
                    html_to_text(Some(&alert.cause_tc))
                } else {
                    String::new()
                },
                issued_time: alert
                    .issued_time
                    .clone()
                    .or_else(|| late_cert.map(|cert| cert.issued_time.clone())),
                details_url,
                has_map: alert.show_map == 1,
                has_shuttle_bus: alert.show_shuttlebus == 1,
                has_alternative_transport: alert.show_alttrans == 1,
            }
        })
        .filter(|alert| !options.active_only || alert.active)
        .filter(|alert| {
            line.as_ref()
                .map(|line| alert.lines.is_empty() || alert.lines.contains(line))
                .unwrap_or(true)
        })
        .collect();

    alerts.sort_by(|a, b| {
        b.active
            .cmp(&a.active)
            .then_with(|| a.severity.rank().cmp(&b.severity.rank()))
            .then_with(|| match (&a.issued_time, &b.issued_time) {
                (Some(a), Some(b)) => b.cmp(a),
                _ => std::cmp::Ordering::Equal,
            })
            .then_with(|| a.kind.cmp(&b.kind))
    });

    let mut counts = AlertCounts::default();
    for alert in &alerts {
        if alert.active {
            counts.active += 1;
        }
        match alert.severity {
            AlertSeverity::Disruption => counts.disruption += 1,
            AlertSeverity::Delay => counts.delay += 1,
            AlertSeverity::Notice => counts.notice += 1,
            AlertSeverity::Record => counts.record += 1,
        }
    }

    Ok(AlertsResult {
        updated_at: Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
        source: "MTR Traffic News (tnews.mtr.com.hk/alert)".to_string(),
        count: alerts.len(),
        network_wide: alerts.iter().filter(|alert| alert.lines.is_empty()).count(),
        counts,
        alerts,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_distinguishes_all_categories() {
        assert_eq!(
            classify_alert_severity(
                "TSI",
                "Service disruption on Tsuen Wan Line",
                false,
                false,
                false
            ),
            AlertSeverity::Disruption
        );
        assert_eq!(
            classify_alert_severity(
                "TSI",
                "Additional waiting time on Island Line",
                false,
                false,
                false
            ),
            AlertSeverity::Delay
        );
        assert_eq!(
            classify_alert_severity("TSI", "Early last train on 28 Nov", false, false, false),
            AlertSeverity::Notice
        );
        assert_eq!(
            classify_alert_severity(
                "LateCert",
                "Train Service Disruption Notification",
                false,
                false,
                false
            ),
            AlertSeverity::Record
        );
        assert_eq!(
            classify_alert_severity("TSI", "Special arrangement", false, true, false),
            AlertSeverity::Disruption
        );
    }
}
