use std::collections::HashMap;

use anyhow::{Result, bail};
use serde::Serialize;

use crate::config::Config;
use crate::csvutil::{num, parse_csv_objects, text};
use crate::http::HttpBackend;
use crate::model::{LrRoute, LrStop};

#[derive(Debug, Clone)]
pub struct LightRailData {
    pub stops: Vec<LrStop>,
    pub routes: Vec<LrRoute>,
    pub stop_by_id: HashMap<i64, usize>,
    pub stop_by_code: HashMap<String, usize>,
    pub route_by_code: HashMap<String, Vec<usize>>,
}

pub fn build_light_rail_data(csv: &str) -> Result<LightRailData> {
    let rows = parse_csv_objects(csv)?;
    let mut stops: Vec<LrStop> = Vec::new();
    let mut stop_by_id: HashMap<i64, usize> = HashMap::new();
    let mut stop_by_code: HashMap<String, usize> = HashMap::new();
    let mut route_map: HashMap<(String, String), Vec<(i64, usize)>> = HashMap::new();

    for row in &rows {
        let Ok(id) = text(row, "Stop ID").parse::<i64>() else {
            continue;
        };
        let route = text(row, "Line Code");
        if route.is_empty() {
            continue;
        }

        let stop_index = if let Some(index) = stop_by_id.get(&id) {
            *index
        } else {
            let index = stops.len();
            stops.push(LrStop {
                id,
                code: text(row, "Stop Code").to_string(),
                name_en: text(row, "English Name").to_string(),
                name_tc: text(row, "Chinese Name").to_string(),
            });
            stop_by_id.insert(id, index);
            index
        };
        stop_by_code.insert(stops[stop_index].code.to_ascii_uppercase(), stop_index);

        let direction = {
            let value = text(row, "Direction");
            if value.is_empty() {
                "1".to_string()
            } else {
                value.to_string()
            }
        };
        let sequence = text(row, "Sequence").parse::<i64>().unwrap_or(0);
        route_map
            .entry((route.to_string(), direction))
            .or_default()
            .push((sequence, stop_index));
    }

    let mut routes: Vec<LrRoute> = route_map
        .into_iter()
        .map(|((route, direction), mut entries)| {
            entries.sort_by_key(|(sequence, _)| *sequence);
            LrRoute {
                route,
                direction,
                stops: entries
                    .into_iter()
                    .map(|(_, index)| stops[index].clone())
                    .collect(),
            }
        })
        .collect();
    routes.sort_by(|a, b| a.route.cmp(&b.route).then(a.direction.cmp(&b.direction)));

    let mut route_by_code: HashMap<String, Vec<usize>> = HashMap::new();
    for (index, route) in routes.iter().enumerate() {
        route_by_code
            .entry(route.route.clone())
            .or_default()
            .push(index);
    }

    Ok(LightRailData {
        stops,
        routes,
        stop_by_id,
        stop_by_code,
        route_by_code,
    })
}

pub async fn load_light_rail(http: &impl HttpBackend, config: &Config) -> Result<LightRailData> {
    let csv = http
        .get_text(&config.datasets.light_rail_routes_and_stops, &[])
        .await?;
    build_light_rail_data(&csv)
}

pub fn find_light_rail_stops(data: &LightRailData, query: &str) -> Vec<LrStop> {
    let raw = query.trim();
    if raw.is_empty() {
        return Vec::new();
    }
    let lower = raw.to_lowercase();
    let code = raw.to_ascii_uppercase();
    let numeric_id = raw.parse::<i64>().ok();

    let mut matches: Vec<(LrStop, i32)> = data
        .stops
        .iter()
        .filter_map(|stop| {
            let score = if numeric_id == Some(stop.id) || stop.code.to_ascii_uppercase() == code {
                100
            } else if stop.name_en.to_lowercase() == lower || stop.name_tc == raw {
                95
            } else if stop.name_en.to_lowercase().contains(&lower) || stop.name_tc.contains(raw) {
                60
            } else {
                0
            };
            (score > 0).then(|| (stop.clone(), score))
        })
        .collect();

    matches.sort_by(|a, b| {
        b.1.cmp(&a.1)
            .then_with(|| a.0.name_en.cmp(&b.0.name_en))
            .then_with(|| a.0.id.cmp(&b.0.id))
    });
    matches.into_iter().map(|(stop, _)| stop).collect()
}

pub fn resolve_lr_stop(data: &LightRailData, query: &str) -> Result<LrStop> {
    let matches = find_light_rail_stops(data, query);
    if matches.is_empty() {
        bail!("No Light Rail stop matched \"{}\".", query.trim());
    }
    let top_name = matches[0].name_en.clone();
    let top: Vec<&LrStop> = matches
        .iter()
        .filter(|stop| stop.name_en == top_name)
        .collect();
    if top.len() > 1 {
        bail!(
            "\"{}\" is ambiguous. Try: {}",
            query.trim(),
            top.iter()
                .take(8)
                .map(|stop| format!("{} ({})", stop.name_en, stop.code))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    Ok(matches[0].clone())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LightRailOctopusFare {
    pub adult: Option<f64>,
    pub child: Option<f64>,
    pub elderly: Option<f64>,
    pub pwd: Option<f64>,
    pub student: Option<f64>,
    pub joy_you_sixty: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LightRailSingleFare {
    pub adult: Option<f64>,
    pub child: Option<f64>,
    pub elderly: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LightRailFare {
    pub from_id: i64,
    pub to_id: i64,
    pub octopus: LightRailOctopusFare,
    pub single: LightRailSingleFare,
}

pub type LightRailFareMatrix = HashMap<String, LightRailFare>;

pub fn build_light_rail_fares(csv: &str) -> Result<LightRailFareMatrix> {
    let mut matrix = HashMap::new();
    for row in parse_csv_objects(csv)? {
        let Ok(from_id) = text(&row, "from_station_id").parse::<i64>() else {
            continue;
        };
        let Ok(to_id) = text(&row, "to_station_id").parse::<i64>() else {
            continue;
        };
        matrix.insert(
            format!("{from_id}:{to_id}"),
            LightRailFare {
                from_id,
                to_id,
                octopus: LightRailOctopusFare {
                    adult: num(row.get("fare_octo_adult")),
                    child: num(row.get("fare_octo_child")),
                    elderly: num(row.get("fare_octo_elderly")),
                    pwd: num(row.get("fare_octo_pwd")),
                    student: num(row.get("fare_octo_student")),
                    joy_you_sixty: num(row.get("fare_octo_joyyou_sixty")),
                },
                single: LightRailSingleFare {
                    adult: num(row.get("fare_single_adult")),
                    child: num(row.get("fare_single_child")),
                    elderly: num(row.get("fare_single_elderly")),
                },
            },
        );
    }
    Ok(matrix)
}

pub async fn load_light_rail_fares(
    http: &impl HttpBackend,
    config: &Config,
) -> Result<LightRailFareMatrix> {
    let csv = http
        .get_text(&config.datasets.light_rail_fares, &[])
        .await?;
    build_light_rail_fares(&csv)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn light_rail_routes_are_ordered_and_numeric_stop_ids_work() {
        let csv = [
            "Line Code,Direction,Stop Code,Stop ID,Chinese Name,English Name,Sequence",
            "507,1,FEP,1,屯門碼頭,Tuen Mun Ferry Pier,1",
            "507,1,TSL,10,田景,Tin King,2",
            "610,2,TSL,10,田景,Tin King,1",
            "610,2,FEP,1,屯門碼頭,Tuen Mun Ferry Pier,2",
        ]
        .join("\n");
        let data = build_light_rail_data(&csv).unwrap();
        assert_eq!(data.stops.len(), 2);
        assert_eq!(data.routes.len(), 2);
        assert_eq!(data.routes[0].stops[0].name_en, "Tuen Mun Ferry Pier");
        assert_eq!(find_light_rail_stops(&data, "1")[0].code, "FEP");
        assert_eq!(find_light_rail_stops(&data, "001")[0].code, "FEP");
        assert_eq!(find_light_rail_stops(&data, "Tin King")[0].id, 10);
    }
}
