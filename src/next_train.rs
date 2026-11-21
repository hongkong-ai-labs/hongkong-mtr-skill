use std::collections::HashMap;

use anyhow::{Result, bail};
use futures::future::join_all;
use serde::Deserialize;

use crate::app::App;
use crate::constants::is_next_train_line;
use crate::http::HttpBackend;
use crate::model::{
    LineRefFull, NextTrain, NextTrainDirection, NextTrainResult, StationNextTrains, StationRef,
    TrainWarning,
};
use crate::network::{line_codes_for_station, resolve_line, resolve_station};

#[derive(Debug, Clone, Deserialize)]
struct RawTrain {
    #[serde(default)]
    seq: String,
    #[serde(default)]
    dest: String,
    #[serde(default)]
    plat: String,
    #[serde(default)]
    time: String,
    #[serde(default)]
    ttnt: String,
    #[serde(default)]
    valid: String,
    #[serde(default)]
    source: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct RawScheduleData {
    #[serde(rename = "UP", default)]
    up: Vec<RawTrain>,
    #[serde(rename = "DOWN", default)]
    down: Vec<RawTrain>,
}

#[derive(Debug, Clone, Deserialize)]
struct RawSchedule {
    #[serde(default)]
    data: Option<HashMap<String, RawScheduleData>>,
    #[serde(rename = "isdelay", default)]
    is_delayed: Option<String>,
    #[serde(default)]
    status: Option<i64>,
    #[serde(default)]
    message: Option<String>,
}

fn normalize_train(network: &crate::model::Network, train: &RawTrain) -> NextTrain {
    let destination = network
        .station_by_code
        .get(&train.dest.to_ascii_uppercase())
        .and_then(|index| network.stations.get(*index));
    NextTrain {
        seq: train.seq.parse::<i64>().unwrap_or(0),
        destination_code: train.dest.clone(),
        destination: destination
            .map(|station| station.name_en.clone())
            .unwrap_or_else(|| train.dest.clone()),
        platform: if train.plat.is_empty() || train.plat == "-" {
            None
        } else {
            Some(train.plat.clone())
        },
        time: train.time.clone(),
        minutes_away: train.ttnt.parse::<i64>().ok(),
        valid: train.valid == "Y",
        source: train.source.clone(),
    }
}

fn eal_disabled(warnings: &[TrainWarning]) -> bool {
    warnings.iter().any(|warning| {
        warning.line == "EAL"
            && (warning.message.contains("NT-205") || warning.message.contains("disabled in CMS"))
    })
}

async fn fetch_line_schedule(
    app: &App<impl HttpBackend>,
    line_code: &str,
    station_code: &str,
) -> Result<RawSchedule> {
    let url = format!(
        "{}?line={}&sta={}&lang=en",
        app.config.endpoints.next_train,
        urlencoding(line_code),
        urlencoding(station_code)
    );
    Ok(app.http.get_json::<RawSchedule>(&url, &[]).await?)
}

fn urlencoding(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.' | '~') {
                character.to_string()
            } else {
                format!("%{:02X}", character as u32)
            }
        })
        .collect()
}

pub async fn get_next_trains(
    app: &App<impl HttpBackend>,
    station_query: &str,
    line_query: Option<&str>,
    direction_filter: Option<&str>,
) -> Result<NextTrainResult> {
    let network = app.network().await?;
    let station = resolve_station(&network, station_query)?;

    let mut line_codes: Vec<String> = line_codes_for_station(&network, station)
        .into_iter()
        .filter(|code| is_next_train_line(code))
        .collect();

    if let Some(line_query) = line_query {
        let line = resolve_line(&network, line_query)?;
        if !station.line_ids.contains(&line.id) {
            bail!(
                "{} does not serve {}. Lines here: {}",
                line.name_en,
                station.name_en,
                line_codes.join(", ")
            );
        }
        if !line.has_real_time {
            bail!(
                "{} does not publish real-time next-train data through MTR's Next Train API.",
                line.name_en
            );
        }
        line_codes = vec![line.code.clone()];
    }

    if line_codes.is_empty() {
        bail!(
            "No real-time next-train data is available for {} ({}).",
            station.name_en,
            line_codes_for_station(&network, station).join(", ")
        );
    }

    let futures = line_codes.iter().map(|code| {
        let code = code.clone();
        let station_code = station.code.clone();
        async move {
            match fetch_line_schedule(app, &code, &station_code).await {
                Ok(raw) if raw.status == Some(1) => Ok((code, raw)),
                Ok(raw) => Err((
                    code,
                    raw.message
                        .unwrap_or_else(|| "No schedule returned".to_string()),
                )),
                Err(error) => Err((code, error.to_string())),
            }
        }
    });
    let results = join_all(futures).await;

    let mut warnings = Vec::new();
    let mut lines: Vec<StationNextTrains> = Vec::new();
    for result in results {
        let (code, raw) = match result {
            Ok(value) => value,
            Err((line, message)) => {
                warnings.push(TrainWarning { line, message });
                continue;
            }
        };

        let Some(line) = network
            .line_by_code
            .get(&code)
            .and_then(|index| network.lines.get(*index))
        else {
            continue;
        };

        let data_key = format!("{code}-{}", station.code);
        let payload = raw
            .data
            .as_ref()
            .and_then(|data| data.get(&data_key).or_else(|| data.values().next()));
        let Some(payload) = payload else {
            warnings.push(TrainWarning {
                line: code,
                message: "Schedule contained no data".to_string(),
            });
            continue;
        };

        let directions: Vec<NextTrainDirection> = [("UP", &payload.up), ("DOWN", &payload.down)]
            .into_iter()
            .map(|(name, trains)| {
                let mut trains: Vec<NextTrain> = trains
                    .iter()
                    .map(|train| normalize_train(&network, train))
                    .collect();
                trains.sort_by_key(|train| train.minutes_away.unwrap_or(999));
                NextTrainDirection {
                    direction: name.to_string(),
                    trains,
                }
            })
            .filter(|direction| !direction.trains.is_empty())
            .filter(|entry| {
                direction_filter
                    .map(|filter| entry.direction.eq_ignore_ascii_case(filter))
                    .unwrap_or(true)
            })
            .collect();

        lines.push(StationNextTrains {
            station: StationRef {
                id: station.id,
                code: station.code.clone(),
                name_en: station.name_en.clone(),
                name_tc: station.name_tc.clone(),
            },
            line: LineRefFull {
                code: line.code.clone(),
                name_en: line.name_en.clone(),
                name_tc: line.name_tc.clone(),
                color: line.color.clone(),
            },
            delayed: raw.is_delayed.as_deref() == Some("Y"),
            message: raw.message,
            directions,
        });
    }

    // MTR sometimes disables the East Rail feed upstream (`NT-205`). Return
    // the warning instead of a hard error so callers can explain the fallback
    // rather than showing nothing.
    let eal_disabled = eal_disabled(&warnings);
    if lines.is_empty() && !eal_disabled {
        bail!(
            "Could not retrieve next trains for {}. {}",
            station.name_en,
            if warnings.is_empty() {
                "All upstream requests failed.".to_string()
            } else {
                warnings
                    .iter()
                    .map(|warning| format!("{}: {}", warning.line, warning.message))
                    .collect::<Vec<_>>()
                    .join("; ")
            }
        );
    }

    Ok(NextTrainResult {
        station: StationRef {
            id: station.id,
            code: station.code.clone(),
            name_en: station.name_en.clone(),
            name_tc: station.name_tc.clone(),
        },
        delayed: lines.iter().any(|line| line.delayed),
        warnings,
        lines,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_only_the_eal_disabled_warning() {
        let warning = |line: &str, message: &str| TrainWarning {
            line: line.to_string(),
            message: message.to_string(),
        };
        assert!(eal_disabled(&[warning(
            "EAL",
            "NT-205 ERL line is disabled in CMS"
        )]));
        assert!(!eal_disabled(&[warning("TML", "NT-205")]));
        assert!(!eal_disabled(&[warning("EAL", "timeout")]));
    }
}
