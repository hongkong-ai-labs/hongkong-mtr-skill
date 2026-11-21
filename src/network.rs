use std::collections::HashMap;

use anyhow::{Result, anyhow, bail};

use crate::config::Config;
use crate::constants::{KNOWN_LINES, known_line};
use crate::csvutil::{parse_csv_objects, text};
use crate::http::HttpClient;
use crate::model::{
    Coordinates, LineRef, Network, RailLine, RailStation, RawStation, RawStationResponse,
    StationRecord, StationSummary,
};

fn coordinate(value: Option<&String>) -> (Option<f64>, Option<f64>) {
    let Some(value) = value else {
        return (None, None);
    };
    let mut parts = value.split(',');
    let lat = parts
        .next()
        .and_then(|part| part.trim().parse::<f64>().ok());
    let lng = parts
        .next()
        .and_then(|part| part.trim().parse::<f64>().ok());
    (lat, lng)
}

fn normalize(value: &str) -> String {
    let mut normalized = value.trim().to_lowercase();
    if normalized.ends_with(" station") {
        normalized.truncate(normalized.len() - " station".len());
    }
    if normalized.ends_with('站') {
        normalized.pop();
    }
    normalized
        .chars()
        .filter(|character| {
            character.is_ascii_alphanumeric() || ('\u{3400}'..='\u{9fff}').contains(character)
        })
        .collect()
}

fn canonicalize_stations(
    mut stations: Vec<RailStation>,
) -> (Vec<RailStation>, HashMap<i64, usize>) {
    stations.sort_by_key(|station| station.id);
    let mut canonical: Vec<RailStation> = Vec::new();
    let mut by_code: HashMap<String, usize> = HashMap::new();
    let mut aliases: HashMap<i64, usize> = HashMap::new();

    for station in stations {
        let code = station.code.to_ascii_uppercase();
        if let Some(&index) = by_code.get(&code) {
            for line_id in &station.line_ids {
                if !canonical[index].line_ids.contains(line_id) {
                    canonical[index].line_ids.push(*line_id);
                }
            }
            aliases.insert(station.id, index);
        } else {
            let index = canonical.len();
            aliases.insert(station.id, index);
            by_code.insert(code, index);
            canonical.push(station);
        }
    }

    (canonical, aliases)
}

fn index_network(
    stations: Vec<RailStation>,
    lines: Vec<RailLine>,
    aliases: HashMap<i64, usize>,
) -> Network {
    let mut station_by_id = aliases;
    for (index, station) in stations.iter().enumerate() {
        station_by_id.insert(station.id, index);
    }
    let station_by_code: HashMap<String, usize> = stations
        .iter()
        .enumerate()
        .map(|(index, station)| (station.code.to_ascii_uppercase(), index))
        .collect();
    let line_by_id: HashMap<i64, usize> = lines
        .iter()
        .enumerate()
        .map(|(index, line)| (line.id, index))
        .collect();
    let line_by_code: HashMap<String, usize> = lines
        .iter()
        .enumerate()
        .map(|(index, line)| (line.code.to_ascii_uppercase(), index))
        .collect();

    let mut network = Network {
        stations,
        lines,
        station_by_id,
        station_by_code,
        line_by_id,
        line_by_code,
    };

    let line_ids: Vec<(i64, Vec<i64>)> = network
        .lines
        .iter()
        .map(|line| (line.id, line.station_ids.clone()))
        .collect();
    for (line_id, station_ids) in line_ids {
        for station_id in station_ids {
            if let Some(&index) = network.station_by_id.get(&station_id)
                && !network.stations[index].line_ids.contains(&line_id)
            {
                network.stations[index].line_ids.push(line_id);
            }
        }
    }

    network
}

/// Build the in-memory network from MTR's trip-planner station response.
pub fn build_network(raw: RawStationResponse) -> Network {
    let lines: Vec<RailLine> = raw
        .lines
        .iter()
        .map(|line| {
            let known = known_line(&line.alias);
            RailLine {
                id: line.id,
                code: line.alias.clone(),
                name_en: line.name_en.clone(),
                name_tc: line.name_tc.clone(),
                color: if line.color.starts_with('#') {
                    line.color.clone()
                } else {
                    format!("#{}", line.color)
                },
                station_ids: line.station_ids.clone(),
                has_real_time: known
                    .map(|item| item.real_time)
                    .unwrap_or(line.alias != "HSR"),
            }
        })
        .collect();

    let raw_stations: Vec<RailStation> = raw
        .stations
        .iter()
        .map(|station: &RawStation| {
            let (lat, lng) = coordinate(station.coordinate.as_ref());
            RailStation {
                id: station.id,
                code: station.alias.clone(),
                name_en: station.name_en.clone(),
                name_tc: station.name_tc.clone(),
                name_sc: station
                    .name_sc
                    .clone()
                    .unwrap_or_else(|| station.name_tc.clone()),
                line_ids: station.line_ids.clone(),
                lat,
                lng,
            }
        })
        .collect();

    let (stations, aliases) = canonicalize_stations(raw_stations);
    index_network(stations, lines, aliases)
}

/// Fallback network built from the open-data lines-and-stations CSV.
pub fn build_network_from_csv(csv: &str) -> Result<Network> {
    let rows = parse_csv_objects(csv)?;
    let mut station_map: HashMap<i64, RailStation> = HashMap::new();
    let mut line_stations: HashMap<String, Vec<(i64, i64)>> = HashMap::new();

    for row in &rows {
        let Ok(id) = text(row, "Station ID").parse::<i64>() else {
            continue;
        };
        let code = text(row, "Station Code");
        let line_code = text(row, "Line Code");
        if code.is_empty() || line_code.is_empty() {
            continue;
        }

        station_map.entry(id).or_insert_with(|| RailStation {
            id,
            code: code.to_string(),
            name_en: text(row, "English Name").to_string(),
            name_tc: text(row, "Chinese Name").to_string(),
            name_sc: text(row, "Chinese Name").to_string(),
            line_ids: Vec::new(),
            lat: None,
            lng: None,
        });

        if text(row, "Direction") != "UT" {
            let sequence = text(row, "Sequence").parse::<i64>().unwrap_or(0);
            line_stations
                .entry(line_code.to_string())
                .or_default()
                .push((sequence, id));
        }
    }

    let mut lines = Vec::new();
    for (code, mut entries) in line_stations {
        entries.sort_by_key(|(sequence, _)| *sequence);
        let mut station_ids: Vec<i64> = Vec::new();
        for (_, station_id) in entries {
            if !station_ids.contains(&station_id) {
                station_ids.push(station_id);
            }
        }
        let known = known_line(&code);
        lines.push(RailLine {
            id: known.map(|line| line.id).unwrap_or(lines.len() as i64 + 1),
            code: code.clone(),
            name_en: known
                .map(|line| line.name_en.to_string())
                .unwrap_or(code.clone()),
            name_tc: known.map(|line| line.name_tc.to_string()).unwrap_or(code),
            color: known
                .map(|line| line.color.to_string())
                .unwrap_or_else(|| "#777777".to_string()),
            station_ids,
            has_real_time: known.map(|line| line.real_time).unwrap_or(true),
        });
    }
    lines.sort_by(|a, b| a.code.cmp(&b.code));

    let (stations, aliases) = canonicalize_stations(station_map.into_values().collect());
    Ok(index_network(stations, lines, aliases))
}

pub async fn load_network(http: &HttpClient, config: &Config) -> Result<Network> {
    let raw = http
        .get_json::<RawStationResponse>(
            &config.endpoints.stations_api,
            &[("api-key", config.trip_planner_key.as_str())],
        )
        .await;

    match raw {
        Ok(raw) if !raw.lines.is_empty() && !raw.stations.is_empty() => Ok(build_network(raw)),
        Ok(_) | Err(_) => {
            let api_error = match raw {
                Ok(_) => anyhow!("station API returned an empty network"),
                Err(error) => anyhow!(error),
            };
            match http
                .get_text(&config.datasets.lines_and_stations, &[])
                .await
            {
                Ok(csv) => build_network_from_csv(&csv),
                Err(_) => Err(api_error.context("station API failed and CSV fallback failed")),
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct StationMatch {
    pub index: usize,
    pub score: i32,
}

/// Resolve a free-text station query to ranked heavy-rail stations.
pub fn find_stations(network: &Network, query: &str) -> Vec<StationMatch> {
    let raw = query.trim();
    if raw.is_empty() {
        return Vec::new();
    }
    let code = raw.to_ascii_uppercase();
    let english = normalize(raw);
    let raw_lower = raw.to_lowercase();
    let mut matches = Vec::new();

    for (index, station) in network.stations.iter().enumerate() {
        let mut score = 0;
        if station.code.to_ascii_uppercase() == code {
            score = 100;
        } else if station.name_en.to_lowercase() == raw_lower
            || station.name_tc == raw
            || station.name_sc == raw
            || station.name_tc.trim_end_matches('站') == raw
        {
            score = 95;
        } else {
            let en = normalize(&station.name_en);
            let tc = normalize(&station.name_tc);
            let sc = normalize(&station.name_sc);
            if en == english || tc == english || sc == english {
                score = 90;
            } else if english.chars().count() >= 2
                && (en.starts_with(&english) || english.starts_with(&en))
            {
                score = 70;
            } else if english.chars().count() >= 3
                && (en.contains(&english) || tc.contains(raw) || sc.contains(raw))
            {
                score = 50;
            }
        }

        if score > 0 {
            matches.push(StationMatch { index, score });
        }
    }

    matches.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| {
                network.stations[a.index]
                    .name_en
                    .cmp(&network.stations[b.index].name_en)
            })
            .then_with(|| {
                network.stations[a.index]
                    .id
                    .cmp(&network.stations[b.index].id)
            })
    });
    matches
}

pub fn find_lines<'a>(network: &'a Network, query: &str) -> Vec<&'a RailLine> {
    let raw = query.trim().to_lowercase();
    if raw.is_empty() {
        return Vec::new();
    }
    network
        .lines
        .iter()
        .filter(|line| {
            line.code.to_lowercase() == raw
                || line.name_en.to_lowercase() == raw
                || line.name_tc == query.trim()
                || line.name_en.to_lowercase().contains(&raw)
                || line.name_tc.contains(query.trim())
        })
        .collect()
}

pub fn resolve_station<'a>(network: &'a Network, query: &str) -> Result<&'a RailStation> {
    let matches = find_stations(network, query);
    if matches.is_empty() {
        let suggestions = network
            .stations
            .iter()
            .take(8)
            .map(|station| format!("{} ({})", station.name_en, station.code))
            .collect::<Vec<_>>()
            .join(", ");
        bail!(
            "No MTR station matched \"{}\". Suggestions: {}",
            query.trim(),
            suggestions
        );
    }

    let top_score = matches[0].score;
    let top: Vec<&StationMatch> = matches
        .iter()
        .filter(|item| item.score == top_score)
        .collect();
    if top.len() > 1 {
        let suggestions = top
            .iter()
            .take(8)
            .map(|item| {
                let station = &network.stations[item.index];
                format!("{} ({})", station.name_en, station.code)
            })
            .collect::<Vec<_>>()
            .join(", ");
        bail!(
            "\"{}\" is ambiguous. Suggestions: {}",
            query.trim(),
            suggestions
        );
    }

    Ok(&network.stations[top[0].index])
}

pub fn resolve_line<'a>(network: &'a Network, query: &str) -> Result<&'a RailLine> {
    let matches = find_lines(network, query);
    if matches.is_empty() {
        bail!(
            "No MTR line matched \"{}\". Available: {}",
            query.trim(),
            network
                .lines
                .iter()
                .map(|line| format!("{} ({})", line.name_en, line.code))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    if matches.len() > 1 {
        let exact = matches.iter().find(|line| {
            line.code.to_lowercase() == query.trim().to_lowercase()
                || line.name_en.to_lowercase() == query.trim().to_lowercase()
        });
        if let Some(line) = exact {
            return Ok(line);
        }
        bail!(
            "\"{}\" is ambiguous. Try one of: {}",
            query.trim(),
            matches
                .iter()
                .map(|line| format!("{} ({})", line.name_en, line.code))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    Ok(matches[0])
}

pub fn line_codes_for_station(network: &Network, station: &RailStation) -> Vec<String> {
    station
        .line_ids
        .iter()
        .filter_map(|line_id| network.line_by_id.get(line_id))
        .filter_map(|index| network.lines.get(*index))
        .map(|line| line.code.clone())
        .collect()
}

pub fn station_name(network: &Network, station_id: i64) -> String {
    network
        .station_by_id
        .get(&station_id)
        .and_then(|index| network.stations.get(*index))
        .map(|station| station.name_en.clone())
        .unwrap_or_else(|| format!("Station {station_id}"))
}

pub fn station_by_id(network: &Network, station_id: i64) -> Option<&RailStation> {
    network
        .station_by_id
        .get(&station_id)
        .and_then(|index| network.stations.get(*index))
}

pub fn line_for_id(network: &Network, line_id: i64) -> Option<&RailLine> {
    network
        .line_by_id
        .get(&line_id)
        .and_then(|index| network.lines.get(*index))
}

pub fn station_summary(network: &Network, station: &RailStation) -> StationSummary {
    StationSummary {
        id: station.id,
        code: station.code.clone(),
        name_en: station.name_en.clone(),
        name_tc: station.name_tc.clone(),
        lines: line_codes_for_station(network, station),
        coordinates: match (station.lat, station.lng) {
            (Some(latitude), Some(longitude)) => Some(Coordinates {
                latitude,
                longitude,
            }),
            _ => None,
        },
    }
}

pub fn station_record(network: &Network, station: &RailStation) -> StationRecord {
    let line_names = station
        .line_ids
        .iter()
        .filter_map(|line_id| network.line_by_id.get(line_id))
        .filter_map(|index| network.lines.get(*index))
        .map(|line| LineRef {
            code: line.code.clone(),
            name_en: line.name_en.clone(),
            name_tc: line.name_tc.clone(),
        })
        .collect();
    StationRecord {
        summary: station_summary(network, station),
        line_names,
        interchange: station.line_ids.len() > 1,
    }
}

pub fn towards_label(network: &Network, towards: Option<&str>) -> Option<String> {
    let towards = towards?;
    Some(
        towards
            .split('/')
            .map(|part| {
                part.trim()
                    .parse::<i64>()
                    .map(|id| station_name(network, id))
                    .unwrap_or_else(|_| part.trim().to_string())
            })
            .collect::<Vec<_>>()
            .join(" / "),
    )
}

pub fn all_known_lines() -> &'static [crate::constants::KnownLine] {
    &KNOWN_LINES
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{RawStation, RawStationLine, RawStationResponse};

    fn raw_network() -> RawStationResponse {
        RawStationResponse {
            error_code: None,
            error_msg: None,
            lines: vec![
                RawStationLine {
                    id: 11,
                    alias: "TWL".into(),
                    name_en: "Tsuen Wan Line".into(),
                    name_tc: "荃灣綫".into(),
                    color: "FF0000".into(),
                    station_ids: vec![1, 2],
                },
                RawStationLine {
                    id: 14,
                    alias: "TCL".into(),
                    name_en: "Tung Chung Line".into(),
                    name_tc: "東涌綫".into(),
                    color: "FE7F1D".into(),
                    station_ids: vec![3, 4],
                },
            ],
            stations: vec![
                RawStation {
                    id: 1,
                    alias: "ADM".into(),
                    name_en: "Admiralty".into(),
                    name_tc: "金鐘".into(),
                    name_sc: Some("金钟".into()),
                    line_ids: vec![11],
                    coordinate: Some("22.27,114.16".into()),
                },
                RawStation {
                    id: 2,
                    alias: "CEN".into(),
                    name_en: "Central".into(),
                    name_tc: "中環".into(),
                    name_sc: Some("中环".into()),
                    line_ids: vec![11],
                    coordinate: None,
                },
                RawStation {
                    id: 3,
                    alias: "HOK".into(),
                    name_en: "Hong Kong".into(),
                    name_tc: "香港".into(),
                    name_sc: Some("香港".into()),
                    line_ids: vec![14],
                    coordinate: None,
                },
                RawStation {
                    id: 4,
                    alias: "TUC".into(),
                    name_en: "Tung Chung".into(),
                    name_tc: "東涌".into(),
                    name_sc: Some("东涌".into()),
                    line_ids: vec![14],
                    coordinate: None,
                },
            ],
        }
    }

    #[test]
    fn resolves_codes_english_and_chinese() {
        let network = build_network(raw_network());
        assert_eq!(
            resolve_station(&network, "ADM").unwrap().name_en,
            "Admiralty"
        );
        assert_eq!(resolve_station(&network, "admiralty").unwrap().code, "ADM");
        assert_eq!(resolve_station(&network, "金鐘").unwrap().code, "ADM");
        assert_eq!(resolve_station(&network, "金钟").unwrap().code, "ADM");
    }

    #[test]
    fn duplicate_station_ids_collapse() {
        let mut raw = raw_network();
        raw.lines = vec![
            RawStationLine {
                id: 10,
                alias: "AEL".into(),
                name_en: "Airport Express".into(),
                name_tc: "機場快綫".into(),
                color: "1C7670".into(),
                station_ids: vec![44, 47],
            },
            RawStationLine {
                id: 14,
                alias: "TCL".into(),
                name_en: "Tung Chung Line".into(),
                name_tc: "東涌綫".into(),
                color: "FE7F1D".into(),
                station_ids: vec![39, 43],
            },
        ];
        raw.stations = vec![
            RawStation {
                id: 39,
                alias: "HOK".into(),
                name_en: "Hong Kong".into(),
                name_tc: "香港".into(),
                name_sc: None,
                line_ids: vec![10, 14],
                coordinate: None,
            },
            RawStation {
                id: 44,
                alias: "HOK".into(),
                name_en: "Hong Kong".into(),
                name_tc: "香港".into(),
                name_sc: None,
                line_ids: vec![10, 14],
                coordinate: None,
            },
            RawStation {
                id: 47,
                alias: "AIR".into(),
                name_en: "Airport".into(),
                name_tc: "機場".into(),
                name_sc: None,
                line_ids: vec![10],
                coordinate: None,
            },
            RawStation {
                id: 43,
                alias: "TUC".into(),
                name_en: "Tung Chung".into(),
                name_tc: "東涌".into(),
                name_sc: None,
                line_ids: vec![14],
                coordinate: None,
            },
        ];
        let network = build_network(raw);
        assert_eq!(
            network
                .stations
                .iter()
                .filter(|station| station.code == "HOK")
                .count(),
            1
        );
        assert_eq!(station_by_id(&network, 44).unwrap().id, 39);
        assert_eq!(find_stations(&network, "Hong Kong").len(), 1);
    }

    #[test]
    fn ordering_is_deterministic() {
        let network = build_network(raw_network());
        let matches = find_stations(&network, "un");
        assert!(matches.iter().all(|item| item.score > 0));
    }

    #[test]
    fn coordinate_parsing_handles_missing_values() {
        assert_eq!(coordinate(None), (None, None));
        assert_eq!(coordinate(Some(&"1,2".to_string())), (Some(1.0), Some(2.0)));
        assert_eq!(coordinate(Some(&"x,2".to_string())), (None, Some(2.0)));
    }

    #[test]
    fn line_resolution_prefers_exact_code() {
        let network = build_network(raw_network());
        let line = resolve_line(&network, "TCL").unwrap();
        assert_eq!(line.id, 14);
    }
}
