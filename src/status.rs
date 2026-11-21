use std::collections::HashMap;

use anyhow::{Result, anyhow, bail};
use chrono::{DateTime, FixedOffset};
use quick_xml::de::from_str;
use serde::{Deserialize, Serialize};

use crate::alerts::{AlertOptions, get_alerts};
use crate::app::App;
use crate::config::Config;
use crate::http::HttpBackend;
use crate::model::{AlertSummary, LineStatus, Network, StationRef, StatusResult, StatusSummary};
use crate::text::html_to_text;

#[derive(Debug, Clone, Deserialize)]
pub struct RawStatus {
    #[serde(rename = "lastBuildDate", default)]
    pub last_build_date: Option<String>,
    #[serde(rename = "refreshInterval", default)]
    pub refresh_interval: Option<String>,
    #[serde(rename = "line", default)]
    lines: Vec<RawStatusLine>,
}

#[derive(Debug, Clone, Deserialize)]
struct RawStatusLine {
    #[serde(rename = "line_code", default)]
    line_code: String,
    #[serde(default)]
    status: String,
    #[serde(rename = "url_en", default)]
    url_en: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct RawRemarks {
    #[serde(rename = "remark_en", default)]
    remark_en: Option<String>,
    #[serde(rename = "remark_tc", default)]
    remark_tc: Option<String>,
    #[serde(rename = "legend_remark", default)]
    legend: Vec<LegendEntry>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LegendEntry {
    #[serde(default)]
    pub color: String,
    #[serde(rename = "name_en", default)]
    pub name_en: String,
    #[serde(rename = "name_tc", default)]
    pub name_tc: String,
}

#[derive(Debug, Clone)]
pub struct LegendData {
    pub by_color: HashMap<String, LegendEntry>,
    pub remark_en: String,
    pub remark_tc: String,
}

#[derive(Debug, Clone, Deserialize)]
struct RawLineConfigResponse {
    #[serde(default)]
    lines: Vec<RawLineConfig>,
}

#[derive(Debug, Clone, Deserialize)]
struct RawLineConfig {
    #[serde(rename = "lineCode", default)]
    line_code: String,
    #[serde(rename = "name_en", default)]
    name_en: String,
    #[serde(rename = "name_tc", default)]
    name_tc: String,
    #[serde(default)]
    color: String,
}

#[derive(Debug, Clone)]
pub struct LineMeta {
    pub name_en: String,
    pub name_tc: String,
    pub color: String,
}

pub type LineConfigMap = HashMap<String, LineMeta>;

pub async fn load_raw_status(http: &impl HttpBackend, config: &Config) -> Result<RawStatus> {
    let text = http.get_text(&config.endpoints.status, &[]).await?;
    from_str(text.trim_start_matches('\u{feff}'))
        .map_err(|error| anyhow!("MTR status feed returned an unexpected document: {error}"))
}

pub async fn load_legend(http: &impl HttpBackend, config: &Config) -> Result<LegendData> {
    let text = http.get_text(&config.endpoints.legend, &[]).await?;
    let remarks: RawRemarks = from_str(text.trim_start_matches('\u{feff}'))
        .map_err(|error| anyhow!("MTR legend feed returned an unexpected document: {error}"))?;
    let by_color = remarks
        .legend
        .into_iter()
        .map(|entry| (entry.color.clone(), entry))
        .collect();
    Ok(LegendData {
        by_color,
        remark_en: html_to_text(remarks.remark_en.as_deref()),
        remark_tc: html_to_text(remarks.remark_tc.as_deref()),
    })
}

pub async fn load_line_config(http: &impl HttpBackend, config: &Config) -> Result<LineConfigMap> {
    let raw: RawLineConfigResponse = http.get_json(&config.endpoints.line_config, &[]).await?;
    Ok(raw
        .lines
        .into_iter()
        .map(|line| {
            let color = if line.color.starts_with('#') {
                line.color
            } else {
                format!("#{}", line.color)
            };
            (
                line.line_code.to_ascii_uppercase(),
                LineMeta {
                    name_en: line.name_en,
                    name_tc: line.name_tc,
                    color,
                },
            )
        })
        .collect())
}

fn details_url(value: Option<&String>) -> Option<String> {
    let value = value?;
    if value.starts_with("http://") || value.starts_with("https://") {
        return Some(value.clone());
    }
    Some(format!(
        "https://www.mtr.com.hk/alert/{}",
        value.trim_start_matches("./")
    ))
}

pub fn to_hkt(value: Option<&String>) -> Option<String> {
    let value = value?;
    let parsed = DateTime::parse_from_rfc2822(value)
        .or_else(|_| DateTime::parse_from_rfc3339(value))
        .ok()?;
    let hkt = FixedOffset::east_opt(8 * 3600)?;
    Some(
        parsed
            .with_timezone(&hkt)
            .format("%d %b %Y, %H:%M")
            .to_string(),
    )
}

pub fn overall_status(statuses: &[String]) -> &'static str {
    if statuses.is_empty() {
        return "normal";
    }
    if statuses.iter().any(|status| status == "typhoon") {
        return "typhoon";
    }
    if statuses.iter().any(|status| status == "red") {
        return "disruption";
    }
    if statuses.iter().any(|status| status == "pink") {
        return "special";
    }
    if statuses.iter().any(|status| status == "yellow") {
        return "delay";
    }
    if statuses.iter().all(|status| status == "grey") {
        return "non-service";
    }
    "normal"
}

pub async fn get_service_status(
    app: &App<impl HttpBackend>,
    line_query: Option<&str>,
) -> Result<StatusResult> {
    let (raw, legend, line_config, alerts) = futures::try_join!(
        app.status(),
        app.legend(),
        app.line_config(),
        get_alerts(
            app,
            AlertOptions {
                include_content: false,
                ..AlertOptions::default()
            }
        ),
    )?;

    if raw.lines.is_empty() {
        bail!("MTR status feed contained no lines");
    }

    let mut lines: Vec<LineStatus> = raw
        .lines
        .iter()
        .map(|line| {
            let code = line.line_code.to_ascii_uppercase();
            let meta = line_config.get(&code);
            let legend_entry = legend.by_color.get(&line.status);
            let line_alerts: Vec<AlertSummary> = alerts
                .alerts
                .iter()
                .filter(|alert| {
                    alert.lines.contains(&code)
                        && (alert.kind != "LateCert" || line.status != "green")
                })
                .map(|alert| alert.summary())
                .collect();

            LineStatus {
                code: code.clone(),
                name_en: meta
                    .map(|meta| meta.name_en.clone())
                    .unwrap_or_else(|| code.clone()),
                name_tc: meta
                    .map(|meta| meta.name_tc.clone())
                    .unwrap_or_else(|| code.clone()),
                color: meta
                    .map(|meta| meta.color.clone())
                    .unwrap_or_else(|| "#777777".to_string()),
                status: line.status.clone(),
                status_label: legend_entry
                    .map(|entry| entry.name_en.clone())
                    .unwrap_or_else(|| line.status.clone()),
                status_label_tc: legend_entry
                    .map(|entry| entry.name_tc.clone())
                    .unwrap_or_else(|| line.status.clone()),
                status_description: legend_entry
                    .map(|entry| entry.name_en.clone())
                    .unwrap_or_else(|| line.status.clone()),
                details_url: details_url(line.url_en.as_ref()),
                updated_at: raw.last_build_date.clone(),
                alerts: line_alerts,
            }
        })
        .collect();

    if let Some(query) = line_query {
        let query = query.trim();
        let lower = query.to_lowercase();
        let matches: Vec<&LineStatus> = lines
            .iter()
            .filter(|line| {
                line.code.to_lowercase() == lower
                    || line.name_en.to_lowercase() == lower
                    || line.name_tc == query
                    || line.name_en.to_lowercase().contains(&lower)
            })
            .collect();
        if matches.is_empty() {
            bail!(
                "No line matched \"{}\". Available: {}",
                query,
                lines
                    .iter()
                    .map(|line| format!("{} ({})", line.name_en, line.code))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
        if matches.len() > 1 {
            bail!(
                "\"{}\" is ambiguous. Try one of: {}",
                query,
                matches
                    .iter()
                    .map(|line| format!("{} ({})", line.name_en, line.code))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
        lines = vec![matches[0].clone()];
    }

    let relevant_alerts: Vec<AlertSummary> = alerts
        .alerts
        .iter()
        .filter(|alert| {
            if alert.lines.is_empty() {
                alert.kind == "TSI"
            } else {
                lines.iter().any(|line| {
                    alert.lines.contains(&line.code)
                        && (alert.kind != "LateCert" || line.status != "green")
                })
            }
        })
        .map(|alert| alert.summary())
        .collect();

    let codes_with = |status: &str| -> Vec<String> {
        lines
            .iter()
            .filter(|line| line.status == status)
            .map(|line| line.code.clone())
            .collect()
    };

    Ok(StatusResult {
        updated_at: raw.last_build_date.clone(),
        updated_at_hkt: to_hkt(raw.last_build_date.as_ref()),
        refresh_seconds: raw
            .refresh_interval
            .as_ref()
            .and_then(|value| value.parse::<i64>().ok())
            .map(|value| value / 1000),
        overall: overall_status(
            &lines
                .iter()
                .map(|line| line.status.clone())
                .collect::<Vec<_>>(),
        )
        .to_string(),
        remark: legend.remark_en.clone(),
        remark_tc: legend.remark_tc.clone(),
        summary: StatusSummary {
            normal_lines: lines.iter().filter(|line| line.status == "green").count(),
            delayed_lines: codes_with("yellow"),
            disrupted_lines: codes_with("red"),
            typhoon_lines: codes_with("typhoon"),
            special_lines: codes_with("pink"),
            non_service_lines: codes_with("grey"),
            notice_count: relevant_alerts.len(),
        },
        lines,
        alerts: relevant_alerts,
    })
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LineMetadata {
    pub code: String,
    pub name_en: String,
    pub name_tc: String,
    pub color: String,
    pub station_count: usize,
    pub real_time: bool,
    pub stations: Vec<StationRef>,
}

pub async fn line_metadata_for_network(
    app: &App<impl HttpBackend>,
    network: &Network,
) -> Result<Vec<LineMetadata>> {
    let config_map = app.line_config().await?;
    Ok(network
        .lines
        .iter()
        .map(|line| {
            let meta = config_map.get(&line.code);
            LineMetadata {
                code: line.code.clone(),
                name_en: meta
                    .map(|meta| meta.name_en.clone())
                    .unwrap_or_else(|| line.name_en.clone()),
                name_tc: meta
                    .map(|meta| meta.name_tc.clone())
                    .unwrap_or_else(|| line.name_tc.clone()),
                color: meta
                    .map(|meta| meta.color.clone())
                    .unwrap_or_else(|| line.color.clone()),
                station_count: line.station_ids.len(),
                real_time: line.has_real_time,
                stations: line
                    .station_ids
                    .iter()
                    .filter_map(|station_id| {
                        network
                            .station_by_id
                            .get(station_id)
                            .and_then(|index| network.stations.get(*index))
                    })
                    .map(|station| StationRef {
                        id: station.id,
                        code: station.code.clone(),
                        name_en: station.name_en.clone(),
                        name_tc: station.name_tc.clone(),
                    })
                    .collect(),
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overall_status_handles_special_and_typhoon() {
        let statuses = |values: &[&str]| {
            values
                .iter()
                .map(|value| value.to_string())
                .collect::<Vec<_>>()
        };
        assert_eq!(overall_status(&statuses(&["green", "green"])), "normal");
        assert_eq!(overall_status(&statuses(&["green", "yellow"])), "delay");
        assert_eq!(overall_status(&statuses(&["yellow", "red"])), "disruption");
        assert_eq!(overall_status(&statuses(&["grey", "grey"])), "non-service");
        assert_eq!(overall_status(&statuses(&["green", "pink"])), "special");
        assert_eq!(overall_status(&statuses(&["red", "typhoon"])), "typhoon");
    }
}
