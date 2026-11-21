use std::collections::HashMap;

use anyhow::Result;

use crate::config::Config;
use crate::csvutil::{num, parse_csv_objects, text};
use crate::http::HttpBackend;
use crate::model::{BusData, BusFare, BusFareSet, BusRouteVariant, BusStop};

pub fn build_bus_data(routes_csv: &str, stops_csv: &str, fares_csv: &str) -> Result<BusData> {
    let mut routes = Vec::new();
    for row in parse_csv_objects(routes_csv)? {
        let route_id = text(&row, "ROUTE_ID");
        if route_id.is_empty() {
            continue;
        }
        routes.push(BusRouteVariant {
            route_id: route_id.to_string(),
            name_en: text(&row, "ROUTE_NAME_ENG").to_string(),
            name_tc: text(&row, "ROUTE_NAME_CHI").to_string(),
            circular: text(&row, "IS_CIRCULAR") == "1",
            line_up: text(&row, "LINE_UP").to_string(),
            line_down: text(&row, "LINE_DOWN").to_string(),
            reference_id: text(&row, "REFERENCE_ID").to_string(),
        });
    }

    let mut stops = Vec::new();
    for row in parse_csv_objects(stops_csv)? {
        let route_id = text(&row, "ROUTE_ID");
        let station_id = text(&row, "STATION_ID");
        if route_id.is_empty() || station_id.is_empty() {
            continue;
        }
        stops.push(BusStop {
            route_id: route_id.to_string(),
            direction: text(&row, "DIRECTION").to_string(),
            sequence: text(&row, "STATION_SEQNO").parse::<i64>().unwrap_or(0),
            id: station_id.to_string(),
            name_en: text(&row, "STATION_NAME_ENG").to_string(),
            name_tc: text(&row, "STATION_NAME_CHI").to_string(),
            lat: num(row.get("STATION_LATITUDE")),
            lng: num(row.get("STATION_LONGITUDE")),
        });
    }

    let mut routes_by_id: HashMap<String, Vec<BusRouteVariant>> = HashMap::new();
    for route in &routes {
        routes_by_id
            .entry(route.route_id.clone())
            .or_default()
            .push(route.clone());
    }

    let mut stops_by_route: HashMap<String, Vec<BusStop>> = HashMap::new();
    for stop in &stops {
        stops_by_route
            .entry(stop.route_id.clone())
            .or_default()
            .push(stop.clone());
    }
    for list in stops_by_route.values_mut() {
        list.sort_by(|a, b| {
            a.direction
                .cmp(&b.direction)
                .then(a.sequence.cmp(&b.sequence))
        });
    }

    let mut fares = HashMap::new();
    for row in parse_csv_objects(fares_csv)? {
        let route_id = text(&row, "ROUTE_ID");
        if route_id.is_empty() {
            continue;
        }
        fares.insert(
            route_id.to_string(),
            BusFare {
                route_id: route_id.to_string(),
                octopus: BusFareSet {
                    adult: num(row.get("FARE_OCTO_ADULT")),
                    child: num(row.get("FARE_OCTO_CHILD")),
                    elderly: num(row.get("FARE_OCTO_ELDERLY")),
                    joy_you: num(row.get("FARE_OCTO_JOYU")),
                    pwd: num(row.get("FARE_OCTO_PWD")),
                    student: num(row.get("FARE_OCTO_STUDENT")),
                },
                single: BusFareSet {
                    adult: num(row.get("FARE_SINGLE_ADULT")),
                    child: num(row.get("FARE_SINGLE_CHILD")),
                    elderly: num(row.get("FARE_SINGLE_ELDERLY")),
                    joy_you: num(row.get("FARE_SINGLE_JOYU")),
                    pwd: num(row.get("FARE_SINGLE_PWD")),
                    student: num(row.get("FARE_SINGLE_STUDENT")),
                },
            },
        );
    }

    Ok(BusData {
        routes,
        stops,
        routes_by_id,
        stops_by_route,
        fares,
    })
}

pub async fn load_bus(http: &impl HttpBackend, config: &Config) -> Result<BusData> {
    let (routes, stops, fares) = futures::join!(
        http.get_text(&config.datasets.bus_routes, &[]),
        http.get_text(&config.datasets.bus_stops, &[]),
        http.get_text(&config.datasets.bus_fares, &[]),
    );
    build_bus_data(&routes?, &stops?, &fares?)
}

pub fn find_bus_routes(data: &BusData, query: &str) -> Vec<BusRouteVariant> {
    let raw = query.trim();
    if raw.is_empty() {
        return Vec::new();
    }
    let lower = raw.to_lowercase();
    let exact: Vec<BusRouteVariant> = data
        .routes
        .iter()
        .filter(|route| {
            route.route_id.to_lowercase() == lower || route.reference_id.to_lowercase() == lower
        })
        .cloned()
        .collect();
    if !exact.is_empty() {
        return exact;
    }

    data.routes
        .iter()
        .filter(|route| {
            route.route_id.to_lowercase().contains(&lower)
                || route.name_en.to_lowercase().contains(&lower)
                || route.name_tc.contains(raw)
        })
        .cloned()
        .collect()
}

pub fn find_bus_stops(data: &BusData, route_id: &str, query: Option<&str>) -> Vec<BusStop> {
    let stops = data
        .stops_by_route
        .get(route_id)
        .cloned()
        .unwrap_or_default();
    let Some(query) = query.map(str::trim).filter(|query| !query.is_empty()) else {
        return stops;
    };
    let lower = query.to_lowercase();
    stops
        .into_iter()
        .filter(|stop| {
            stop.id.to_lowercase().contains(&lower)
                || stop.name_en.to_lowercase().contains(&lower)
                || stop.name_tc.contains(query)
        })
        .collect()
}

pub fn bus_fare_for_route<'a>(data: &'a BusData, route_id: &str) -> Option<&'a BusFare> {
    data.fares.get(route_id)
}

pub fn direction_description(data: &BusData, route_id: &str, direction: &str) -> String {
    let stops: Vec<&BusStop> = data
        .stops_by_route
        .get(route_id)
        .map(|stops| {
            stops
                .iter()
                .filter(|stop| stop.direction == direction)
                .collect()
        })
        .unwrap_or_default();
    if stops.is_empty() {
        return direction.to_string();
    }
    format!(
        "{} → {}",
        stops
            .first()
            .map(|stop| stop.name_en.as_str())
            .unwrap_or(""),
        stops.last().map(|stop| stop.name_en.as_str()).unwrap_or("")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bus_data_indexes_routes_stops_and_fares() {
        let routes = [
            "ROUTE_ID,ROUTE_NAME_CHI,ROUTE_NAME_ENG,IS_CIRCULAR,LINE_UP,LINE_DOWN,REFERENCE_ID",
            "K12,大埔墟站至八號花園,Tai Po Market Station to Eightland Garden,0,K12_EG,K12_TPMS,K12",
        ].join("\n");
        let stops = [
            "ROUTE_ID,DIRECTION,STATION_SEQNO,STATION_ID,STATION_LATITUDE,STATION_LONGITUDE,STATION_NAME_CHI,STATION_NAME_ENG,REFERENCE_ID",
            "K12,O,1,K12-U010,22.44,114.16,大埔墟站,Tai Po Market Station,K12",
            "K12,O,2,K12-U020,22.45,114.17,八號花園,Eightland Garden,K12",
            "K12,I,1,K12-D010,22.45,114.17,八號花園,Eightland Garden,K12",
            "K12,I,2,K12-D020,22.44,114.16,大埔墟站,Tai Po Market Station,K12",
        ].join("\n");
        let fares = [
            "ROUTE_ID,FARE_OCTO_ADULT,FARE_OCTO_CHILD,FARE_OCTO_ELDERLY,FARE_OCTO_JOYU,FARE_OCTO_PWD,FARE_OCTO_STUDENT,FARE_SINGLE_ADULT,FARE_SINGLE_CHILD,FARE_SINGLE_ELDERLY,FARE_SINGLE_JOYU,FARE_SINGLE_PWD,FARE_SINGLE_STUDENT,REFERENCE_ID",
            "K12,4.8,2.4,2,2,2,4.8,4.8,2.4,2.4,-,-,-,K12",
        ].join("\n");
        let data = build_bus_data(&routes, &stops, &fares).unwrap();
        assert_eq!(find_bus_routes(&data, "K12").len(), 1);
        assert_eq!(find_bus_routes(&data, "Tai Po Market Station").len(), 1);
        assert_eq!(data.stops_by_route.get("K12").unwrap().len(), 4);
        assert_eq!(data.stops_by_route.get("K12").unwrap()[0].direction, "I");
        assert_eq!(
            bus_fare_for_route(&data, "K12").unwrap().octopus.adult,
            Some(4.8)
        );
    }
}
