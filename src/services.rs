use std::collections::{HashMap, HashSet};

use anyhow::{Result, bail};
use futures::future::join_all;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::app::App;
use crate::bus::{bus_fare_for_route, direction_description, find_bus_routes, find_bus_stops};
use crate::fares::fare_key;
use crate::http::HttpBackend;
use crate::light_rail::{find_light_rail_stops, resolve_lr_stop};
use crate::model::{
    BusArrival, BusEta, BusNextResult, BusServiceStatus, BusStop, LrNextResult, LrPlatform,
    LrPlatformRoute, LrRoute, LrStop, StationFacilities,
};
use crate::network::{line_codes_for_station, resolve_station, station_record};
use crate::status::line_metadata_for_network;
use crate::text::{first_number, html_to_text};

pub async fn search_stations(app: &App<impl HttpBackend>, query: &str, limit: u8) -> Result<Value> {
    let network = app.network().await?;
    let matches: Vec<Value> = crate::network::find_stations(&network, query)
        .into_iter()
        .take(limit.clamp(1, 20) as usize)
        .map(|item| serde_json::to_value(station_record(&network, &network.stations[item.index])))
        .collect::<std::result::Result<_, _>>()?;
    Ok(json!({
        "query": query,
        "count": matches.len(),
        "stations": matches,
    }))
}

pub async fn list_lines(app: &App<impl HttpBackend>, include_stations: bool) -> Result<Value> {
    let network = app.network().await?;
    let light_rail = app.light_rail().await?;
    let metadata = line_metadata_for_network(app, &network).await?;

    let lines: Vec<Value> = metadata
        .iter()
        .map(|line| {
            let mut value = serde_json::to_value(line)?;
            if !include_stations {
                value
                    .as_object_mut()
                    .map(|object| object.remove("stations"));
            }
            Ok(value)
        })
        .collect::<std::result::Result<_, serde_json::Error>>()?;

    let mut lr = json!({
        "code": "LR",
        "nameEn": "Light Rail",
        "nameTc": "輕鐵",
        "color": "#9F7A00",
        "realTime": true,
        "routeCount": light_rail.route_by_code.len(),
        "directionCount": light_rail.routes.len(),
        "stopCount": light_rail.stops.len(),
    });
    if include_stations {
        lr["routes"] = Value::Array(
            light_rail
                .routes
                .iter()
                .map(|route| {
                    json!({
                        "route": route.route,
                        "direction": route.direction,
                        "origin": route.stops.first().map(|stop| stop.name_en.clone()),
                        "destination": route.stops.last().map(|stop| stop.name_en.clone()),
                        "stops": route.stops.iter().enumerate().map(|(index, stop)| json!({
                            "sequence": index + 1,
                            "code": stop.code,
                            "id": stop.id,
                            "nameEn": stop.name_en,
                            "nameTc": stop.name_tc,
                        })).collect::<Vec<_>>(),
                    })
                })
                .collect(),
        );
    }

    Ok(json!({
        "count": lines.len(),
        "lines": lines,
        "lightRail": lr,
    }))
}

pub async fn get_station_facilities(
    app: &App<impl HttpBackend>,
    station_query: &str,
) -> Result<Value> {
    let network = app.network().await?;
    let station = resolve_station(&network, station_query)?;
    let facilities = app.facilities().await?;
    let mut result: StationFacilities =
        crate::facilities::facilities_for_station(&facilities, station);
    result.lines = line_codes_for_station(&network, station);
    Ok(serde_json::to_value(result)?)
}

pub async fn get_rail_fares(
    app: &App<impl HttpBackend>,
    from_query: &str,
    to_query: &str,
) -> Result<Value> {
    let network = app.network().await?;
    let from = resolve_station(&network, from_query)?;
    let to = resolve_station(&network, to_query)?;
    let matrix = app.rail_fares().await?;
    let fare = matrix.get(&fare_key(from.id, to.id));

    if let Some(fare) = fare {
        return Ok(json!({
            "mode": "heavy-rail",
            "from": { "code": from.code, "nameEn": from.name_en, "nameTc": from.name_tc, "id": from.id },
            "to": { "code": to.code, "nameEn": to.name_en, "nameTc": to.name_tc, "id": to.id },
            "currency": "HKD",
            "source": "MTR fares open data (mtr_lines_fares.csv)",
            "fares": {
                "octopus": {
                    "adult": fare.octopus.adult,
                    "student": fare.octopus.student,
                    "child": fare.octopus.child,
                    "elderly": fare.octopus.elderly,
                    "joyYouSixty": fare.octopus.joy_you_sixty,
                    "pwd": fare.octopus.pwd,
                },
                "singleJourney": {
                    "adult": fare.single.adult,
                    "child": fare.single.child,
                    "elderly": fare.single.elderly,
                },
            },
        }));
    }

    // The open-data matrix excludes Airport Express and HSR; use the trip
    // planner (which includes AEL fares) as a fallback.
    let journey = crate::journey::plan_journey(app, from_query, to_query, Some(1)).await?;
    let route = journey
        .routes
        .first()
        .ok_or_else(|| anyhow::anyhow!("No fare found from {} to {}.", from.name_en, to.name_en))?;

    Ok(json!({
        "mode": "heavy-rail",
        "from": { "code": from.code, "nameEn": from.name_en, "nameTc": from.name_tc, "id": from.id },
        "to": { "code": to.code, "nameEn": to.name_en, "nameTc": to.name_tc, "id": to.id },
        "currency": "HKD",
        "source": "MTR trip planner (fare matrix does not cover Airport Express)",
        "estimatedJourneyMinutes": route.time_minutes,
        "fares": {
            "octopus": {
                "adult": route.fares.octopus_adult,
                "student": route.fares.octopus_student,
                "child": route.fares.octopus_child,
                "elderly": route.fares.octopus_elderly,
                "joyYouSixty": route.fares.octopus_joy_you,
                "pwd": null,
            },
            "singleJourney": {
                "adult": route.fares.single_adult,
                "child": route.fares.single_child,
                "elderly": route.fares.single_elderly,
            },
            "firstClassOctopusAdult": route.fares.first_class_octopus_adult,
        },
    }))
}

pub async fn get_light_rail_fares(
    app: &App<impl HttpBackend>,
    from_query: &str,
    to_query: &str,
) -> Result<Value> {
    let data = app.light_rail().await?;
    let from = resolve_lr_stop(&data, from_query)?;
    let to = resolve_lr_stop(&data, to_query)?;
    let matrix = app.light_rail_fares().await?;
    let fare = matrix
        .get(&format!("{}:{}", from.id, to.id))
        .ok_or_else(|| {
            anyhow::anyhow!(
                "No Light Rail fare found from {} to {}.",
                from.name_en,
                to.name_en
            )
        })?;

    Ok(json!({
        "mode": "light-rail",
        "from": from,
        "to": to,
        "currency": "HKD",
        "source": "MTR Light Rail fares open data (light_rail_fares.csv)",
        "fares": {
            "octopus": fare.octopus,
            "singleJourney": fare.single,
        },
    }))
}

#[derive(Debug, Clone, Deserialize)]
struct RawLrRoute {
    #[serde(default)]
    train_length: i64,
    #[serde(rename = "arrival_departure", default)]
    arrival_departure: String,
    #[serde(rename = "dest_en", default)]
    dest_en: String,
    #[serde(rename = "dest_ch", default)]
    dest_ch: String,
    #[serde(rename = "additionalInfo1", default)]
    additional_info1: String,
    #[serde(rename = "routeRemarkEng2", default)]
    route_remark_eng: String,
    #[serde(rename = "routeRemarkChi2", default)]
    route_remark_chi: String,
    #[serde(rename = "time_en", default)]
    time_en: String,
    #[serde(rename = "time_ch", default)]
    time_ch: String,
    #[serde(default)]
    special: i64,
    #[serde(rename = "route_no", default)]
    route_no: String,
}

#[derive(Debug, Clone, Deserialize)]
struct RawLrPlatform {
    #[serde(rename = "platform_id", default)]
    platform_id: i64,
    #[serde(rename = "route_list", default)]
    route_list: Option<Vec<RawLrRoute>>,
}

#[derive(Debug, Clone, Deserialize)]
struct RawLrResponse {
    #[serde(default)]
    status: Option<i64>,
    #[serde(rename = "system_time", default)]
    system_time: Option<String>,
    #[serde(rename = "platform_list", default)]
    platform_list: Option<Vec<RawLrPlatform>>,
}

pub async fn get_light_rail_next_trains(
    app: &App<impl HttpBackend>,
    stop_query: &str,
    route_filter: Option<&str>,
) -> Result<Value> {
    let data = app.light_rail().await?;
    let stop = resolve_lr_stop(&data, stop_query)?;
    let url = format!(
        "{}?station_id={}&with_special=1",
        app.config.endpoints.light_rail, stop.id
    );
    let raw: RawLrResponse = app.http.get_json(&url, &[]).await?;
    if raw.status.is_some() && raw.status != Some(1) {
        bail!("MTR Light Rail API returned an error status.");
    }

    let route_filter = route_filter.map(|route| route.trim().to_ascii_uppercase());
    let platforms: Vec<LrPlatform> = raw
        .platform_list
        .unwrap_or_default()
        .into_iter()
        .map(|platform| {
            let mut routes: Vec<LrPlatformRoute> = platform
                .route_list
                .unwrap_or_default()
                .into_iter()
                .filter(|route| {
                    route_filter
                        .as_ref()
                        .map(|filter| route.route_no.to_ascii_uppercase() == *filter)
                        .unwrap_or(true)
                })
                .map(|route| {
                    let remark = if !route.route_remark_eng.is_empty() {
                        Some(route.route_remark_eng)
                    } else if !route.additional_info1.is_empty() {
                        Some(route.additional_info1.clone())
                    } else {
                        None
                    };
                    LrPlatformRoute {
                        route: route.route_no,
                        destination: route.dest_en,
                        destination_tc: route.dest_ch,
                        platform: platform.platform_id,
                        arrival_departure: if route.arrival_departure == "A" {
                            "arrival".to_string()
                        } else {
                            "departure".to_string()
                        },
                        minutes_away: first_number(&route.time_en),
                        time_text: route.time_en,
                        time_text_tc: route.time_ch,
                        special: route.special == 1,
                        train_length: route.train_length,
                        remark,
                        remark_tc: (!route.route_remark_chi.is_empty())
                            .then_some(route.route_remark_chi),
                        additional_info: (!route.additional_info1.is_empty())
                            .then_some(route.additional_info1),
                    }
                })
                .collect();
            routes.sort_by_key(|route| route.minutes_away.unwrap_or(999));
            LrPlatform {
                platform_id: platform.platform_id,
                routes,
            }
        })
        .filter(|platform| !platform.routes.is_empty())
        .collect();

    let total: usize = platforms.iter().map(|platform| platform.routes.len()).sum();
    if total == 0 {
        bail!(
            "{}",
            match route_filter {
                Some(route) => format!(
                    "No {route} trains are currently predicted at {}.",
                    stop.name_en
                ),
                None => format!(
                    "No Light Rail predictions are currently available for {}.",
                    stop.name_en
                ),
            }
        );
    }

    Ok(serde_json::to_value(LrNextResult {
        stop,
        system_time: raw.system_time,
        platforms,
    })?)
}

fn routes_serving_stop(
    data: &crate::light_rail::LightRailData,
    stop: &LrStop,
    query: &str,
) -> Value {
    let mut routes: Vec<Value> = data
        .routes
        .iter()
        .filter_map(|route| {
            let index = route
                .stops
                .iter()
                .position(|candidate| candidate.id == stop.id)?;
            let origin = route
                .stops
                .first()
                .map(|candidate| candidate.name_en.clone());
            let destination = route
                .stops
                .last()
                .map(|candidate| candidate.name_en.clone());
            Some(json!({
                "route": route.route,
                "direction": route.direction,
                "directionName": match (&origin, &destination) {
                    (Some(origin), Some(destination)) => Some(format!("{origin} → {destination}")),
                    _ => None,
                },
                "origin": origin,
                "destination": destination,
                "stopSequence": index + 1,
            }))
        })
        .collect();
    routes.sort_by(|a, b| {
        a["route"]
            .as_str()
            .cmp(&b["route"].as_str())
            .then_with(|| a["direction"].as_str().cmp(&b["direction"].as_str()))
    });
    json!({
        "query": query,
        "matched": "stop",
        "stop": stop,
        "routes": routes,
    })
}

fn route_detail(route: &LrRoute) -> Value {
    let origin = route.stops.first().map(|stop| stop.name_en.clone());
    let destination = route.stops.last().map(|stop| stop.name_en.clone());
    json!({
        "route": route.route,
        "direction": route.direction,
        "directionName": match (&origin, &destination) {
            (Some(origin), Some(destination)) => Some(format!("{origin} → {destination}")),
            _ => None,
        },
        "origin": origin,
        "destination": destination,
        "stopCount": route.stops.len(),
        "stops": route.stops.iter().enumerate().map(|(index, stop)| json!({
            "sequence": index + 1,
            "code": stop.code,
            "id": stop.id,
            "nameEn": stop.name_en,
            "nameTc": stop.name_tc,
        })).collect::<Vec<_>>(),
    })
}

pub async fn get_light_rail_routes(
    app: &App<impl HttpBackend>,
    query: Option<&str>,
) -> Result<Value> {
    let data = app.light_rail().await?;
    let Some(query) = query.map(str::trim).filter(|query| !query.is_empty()) else {
        let mut routes: Vec<Value> = data
            .route_by_code
            .iter()
            .map(|(route, variants)| {
                json!({
                    "route": route,
                    "directions": variants.iter().map(|index| {
                        let variant = &data.routes[*index];
                        let origin = variant.stops.first().map(|stop| stop.name_en.clone());
                        let destination = variant.stops.last().map(|stop| stop.name_en.clone());
                        json!({
                            "direction": variant.direction,
                            "directionName": match (&origin, &destination) {
                                (Some(origin), Some(destination)) => Some(format!("{origin} → {destination}")),
                                _ => None,
                            },
                            "origin": origin,
                            "destination": destination,
                            "stopCount": variant.stops.len(),
                        })
                    }).collect::<Vec<_>>(),
                })
            })
            .collect();
        routes.sort_by(|a, b| a["route"].as_str().cmp(&b["route"].as_str()));
        return Ok(json!({ "routeCount": data.route_by_code.len(), "routes": routes }));
    };

    let route_code = query.to_ascii_uppercase();
    let mut matching = data.route_by_code.get(&route_code).cloned();

    if matching.is_none()
        && let Ok(stop_id) = query.parse::<i64>()
        && let Some(stop) = data
            .stop_by_id
            .get(&stop_id)
            .and_then(|index| data.stops.get(*index))
    {
        return Ok(routes_serving_stop(&data, stop, query));
    }

    if matching.is_none() {
        let candidates: Vec<&str> = data
            .route_by_code
            .keys()
            .filter(|code| code.starts_with(&route_code) || code.contains(&route_code))
            .map(String::as_str)
            .collect();
        if candidates.len() == 1 {
            matching = data.route_by_code.get(candidates[0]).cloned();
        } else if candidates.len() > 1 {
            bail!(
                "Light Rail route \"{}\" is ambiguous. Matches: {}.",
                query,
                candidates.join(", ")
            );
        }
    }

    if let Some(indices) = matching {
        return Ok(json!({
            "query": query,
            "matched": "route",
            "routes": indices.iter().map(|index| route_detail(&data.routes[*index])).collect::<Vec<_>>(),
        }));
    }

    let stops = find_light_rail_stops(&data, query);
    if stops.is_empty() {
        bail!("No Light Rail route or stop matched \"{}\".", query);
    }
    Ok(routes_serving_stop(&data, &stops[0], query))
}

#[derive(Debug, Clone, Deserialize)]
struct RawBusArrival {
    #[serde(rename = "busId", default)]
    bus_id: String,
    #[serde(rename = "departureTimeText", default)]
    departure_time_text: String,
    #[serde(rename = "isDelayed", default)]
    is_delayed: String,
    #[serde(rename = "isScheduled", default)]
    is_scheduled: String,
    #[serde(rename = "lineRef", default)]
    line_ref: String,
}

#[derive(Debug, Clone, Deserialize)]
struct RawBusStop {
    #[serde(default)]
    bus: Option<Vec<RawBusArrival>>,
    #[serde(rename = "busStopId", default)]
    bus_stop_id: String,
    #[serde(rename = "isSuspended", default)]
    is_suspended: String,
}

#[derive(Debug, Clone, Deserialize)]
struct RawBusResponse {
    #[serde(rename = "routeStatus", default)]
    route_status: Option<String>,
    #[serde(rename = "routeStatusColour", default)]
    route_status_colour: Option<String>,
    #[serde(rename = "routeStatusRemarkTitle", default)]
    route_status_remark_title: Option<String>,
    #[serde(rename = "routeStatusRemarkContent", default)]
    route_status_remark_content: Option<String>,
    #[serde(rename = "routeStatusTime", default)]
    route_status_time: Option<String>,
    #[serde(rename = "appRefreshTimeInSecond", default)]
    app_refresh_time_seconds: Option<String>,
    #[serde(rename = "busStop", default)]
    bus_stop: Option<Vec<RawBusStop>>,
    #[serde(rename = "footerRemarks", default)]
    footer_remarks: Option<String>,
}

async fn fetch_route_schedule(
    app: &App<impl HttpBackend>,
    route_name: &str,
) -> Result<RawBusResponse> {
    let body = json!({ "language": "en", "routeName": route_name });
    Ok(app
        .http
        .post_json(
            &app.config.endpoints.bus,
            &body,
            &[("Content-Type", "application/json")],
        )
        .await?)
}

pub async fn get_bus_next_trains(
    app: &App<impl HttpBackend>,
    route_query: &str,
    stop_query: Option<&str>,
) -> Result<Value> {
    let data = app.bus().await?;
    let variants = find_bus_routes(&data, route_query);
    if variants.is_empty() {
        bail!(
            "No MTR Bus route matched \"{}\". Routes include 506, K12, K14, K17, K18, K51, K52, K53, K54, K58, K65, K66, K68, K73, K74, K75, K76.",
            route_query
        );
    }

    let route_ids: Vec<String> = {
        let mut seen = HashSet::new();
        variants
            .iter()
            .filter(|variant| seen.insert(variant.route_id.clone()))
            .map(|variant| variant.route_id.clone())
            .collect()
    };

    let requests: Vec<(String, String)> = route_ids
        .iter()
        .flat_map(|route_id| {
            let route_variants = data.routes_by_id.get(route_id).cloned().unwrap_or_default();
            let mut names: Vec<String> = route_variants
                .iter()
                .map(|variant| {
                    if variant.reference_id.is_empty() {
                        variant.route_id.clone()
                    } else {
                        variant.reference_id.clone()
                    }
                })
                .collect();
            names.sort();
            names.dedup();
            if names.is_empty() {
                names.push(route_id.clone());
            }
            names
                .into_iter()
                .map(|name| (route_id.clone(), name))
                .collect::<Vec<_>>()
        })
        .collect();

    let results = join_all(requests.iter().map(|(route_id, requested)| async move {
        (
            route_id.clone(),
            requested.clone(),
            fetch_route_schedule(app, requested).await,
        )
    }))
    .await;

    let mut schedules = Vec::new();
    let mut first_error = None;
    for (route_id, requested, result) in results {
        match result {
            Ok(response) => schedules.push((route_id, requested, response)),
            Err(error) => {
                if first_error.is_none() {
                    first_error = Some(error.to_string());
                }
            }
        }
    }

    if schedules.is_empty() {
        bail!(
            "MTR Bus returned no schedule for {}. {}",
            route_ids.join("/"),
            first_error.unwrap_or_default()
        );
    }

    let stop_lookup: HashMap<&str, &BusStop> = data
        .stops
        .iter()
        .map(|stop| (stop.id.as_str(), stop))
        .collect();
    let mut stops: Vec<BusEta> = Vec::new();

    for (_, _, response) in &schedules {
        for raw_stop in response.bus_stop.as_deref().unwrap_or(&[]) {
            let Some(meta) = stop_lookup.get(raw_stop.bus_stop_id.as_str()) else {
                continue;
            };
            if let Some(query) = stop_query.map(str::trim).filter(|query| !query.is_empty()) {
                let lower = query.to_lowercase();
                if !meta.name_en.to_lowercase().contains(&lower)
                    && !meta.name_tc.contains(query)
                    && !meta.id.to_lowercase().contains(&lower)
                {
                    continue;
                }
            }

            stops.push(BusEta {
                route: meta.route_id.clone(),
                direction: direction_description(&data, &meta.route_id, &meta.direction),
                direction_code: meta.direction.clone(),
                stop_id: meta.id.clone(),
                stop_name_en: Some(meta.name_en.clone()),
                stop_name_tc: Some(meta.name_tc.clone()),
                sequence: Some(meta.sequence),
                suspended: raw_stop.is_suspended == "1",
                buses: raw_stop
                    .bus
                    .as_deref()
                    .unwrap_or(&[])
                    .iter()
                    .map(|bus| BusArrival {
                        bus_id: bus.bus_id.clone(),
                        minutes_away: first_number(&bus.departure_time_text),
                        departure_text: bus.departure_time_text.clone(),
                        delayed: bus.is_delayed == "1",
                        scheduled: bus.is_scheduled == "1",
                        line_ref: bus.line_ref.clone(),
                    })
                    .collect(),
            });
        }
    }

    if stop_query
        .map(str::trim)
        .is_some_and(|query| !query.is_empty())
        && stops.is_empty()
    {
        bail!(
            "No stop on route {} matched \"{}\".",
            route_ids.join("/"),
            stop_query.unwrap_or_default()
        );
    }

    let mut seen_stops = HashSet::new();
    stops.retain(|stop| seen_stops.insert(format!("{}|{}", stop.stop_id, stop.direction)));

    let service_status: Vec<BusServiceStatus> = schedules
        .iter()
        .map(|(route_id, requested, response)| BusServiceStatus {
            route_id: route_id.clone(),
            variant: requested.clone(),
            status: response.route_status.clone(),
            status_colour: response.route_status_colour.clone(),
            status_title: response.route_status_remark_title.clone(),
            status_remark: {
                let text = html_to_text(response.route_status_remark_content.as_deref());
                (!text.is_empty()).then_some(text)
            },
            updated_at: response.route_status_time.clone(),
            refresh_seconds: response
                .app_refresh_time_seconds
                .as_ref()
                .and_then(|value| value.parse::<i64>().ok()),
        })
        .collect();

    let fares: HashMap<String, Option<crate::model::BusFare>> = route_ids
        .iter()
        .map(|route_id| {
            (
                route_id.clone(),
                bus_fare_for_route(&data, route_id).cloned(),
            )
        })
        .collect();

    let result = BusNextResult {
        route: route_ids.join(" / "),
        service_status,
        suspended_stop_count: stops.iter().filter(|stop| stop.suspended).count(),
        variants: variants.clone(),
        fares,
        stop_count: stops.len(),
        stops,
        remark: schedules
            .first()
            .and_then(|(_, _, response)| response.footer_remarks.clone())
            .unwrap_or_else(|| {
                "Arrival times are MTR Bus estimates and may change with traffic.".to_string()
            }),
    };
    Ok(serde_json::to_value(result)?)
}

fn bus_route_details(
    data: &crate::model::BusData,
    route_ids: &[String],
    stop_filter: Option<&str>,
) -> Vec<Value> {
    route_ids
        .iter()
        .map(|route_id| {
            let route_variants = data.routes_by_id.get(route_id).cloned().unwrap_or_default();
            let all_stops = data.stops_by_route.get(route_id).cloned().unwrap_or_default();
            let mut directions: Vec<Value> = Vec::new();
            let mut seen_directions = HashSet::new();
            for stop in &all_stops {
                if !seen_directions.insert(stop.direction.clone()) {
                    continue;
                }
                let list: Vec<&BusStop> = all_stops
                    .iter()
                    .filter(|candidate| candidate.direction == stop.direction)
                    .collect();
                directions.push(json!({
                    "code": stop.direction,
                    "name": format!(
                        "{} → {}",
                        list.first().map(|candidate| candidate.name_en.as_str()).unwrap_or(""),
                        list.last().map(|candidate| candidate.name_en.as_str()).unwrap_or("")
                    ),
                    "stopCount": list.len(),
                }));
            }

            let stops = find_bus_stops(data, route_id, stop_filter);
            json!({
                "routeId": route_id,
                "variants": route_variants,
                "directions": directions,
                "fare": bus_fare_for_route(data, route_id),
                "stopCount": stops.len(),
                "totalStopCount": all_stops.len(),
                "stops": stops.iter().map(|stop| json!({
                    "direction": direction_description(data, route_id, &stop.direction),
                    "directionCode": stop.direction,
                    "sequence": stop.sequence,
                    "id": stop.id,
                    "nameEn": stop.name_en,
                    "nameTc": stop.name_tc,
                    "coordinates": match (stop.lat, stop.lng) {
                        (Some(latitude), Some(longitude)) => Some(json!({ "latitude": latitude, "longitude": longitude })),
                        _ => None,
                    },
                })).collect::<Vec<_>>(),
            })
        })
        .collect()
}

pub async fn get_bus_routes(
    app: &App<impl HttpBackend>,
    query: Option<&str>,
    stop_filter: Option<&str>,
) -> Result<Value> {
    let data = app.bus().await?;
    let Some(query) = query.map(str::trim).filter(|query| !query.is_empty()) else {
        let mut routes: Vec<Value> = data
            .routes_by_id
            .iter()
            .map(|(route_id, variants)| {
                json!({
                    "routeId": route_id,
                    "variants": variants,
                    "fare": bus_fare_for_route(&data, route_id),
                })
            })
            .collect();
        routes.sort_by(|a, b| a["routeId"].as_str().cmp(&b["routeId"].as_str()));
        return Ok(json!({ "routeCount": data.routes_by_id.len(), "routes": routes }));
    };

    let variants = find_bus_routes(&data, query);
    if !variants.is_empty() {
        let mut route_ids: Vec<String> = variants
            .iter()
            .map(|variant| variant.route_id.clone())
            .collect();
        route_ids.sort();
        route_ids.dedup();
        return Ok(json!({
            "query": query,
            "matched": "routes",
            "routeCount": route_ids.len(),
            "routes": bus_route_details(&data, &route_ids, stop_filter),
        }));
    }

    let lower = query.to_lowercase();
    let stop_matches: Vec<&BusStop> = data
        .stops
        .iter()
        .filter(|stop| {
            stop.name_en.to_lowercase().contains(&lower)
                || stop.name_tc.contains(query)
                || stop.id.to_lowercase().contains(&lower)
        })
        .take(20)
        .collect();

    if stop_matches.is_empty() {
        bail!(
            "No MTR Bus route or stop matched \"{}\". Try a route number (506, K12) or a stop name.",
            query
        );
    }

    let mut route_ids: Vec<String> = stop_matches
        .iter()
        .map(|stop| stop.route_id.clone())
        .collect();
    route_ids.sort();
    route_ids.dedup();

    Ok(json!({
        "query": query,
        "matched": "stops",
        "stopCount": stop_matches.len(),
        "stopMatches": stop_matches.iter().map(|stop| json!({
            "routeId": stop.route_id,
            "direction": direction_description(&data, &stop.route_id, &stop.direction),
            "directionCode": stop.direction,
            "sequence": stop.sequence,
            "id": stop.id,
            "nameEn": stop.name_en,
            "nameTc": stop.name_tc,
        })).collect::<Vec<_>>(),
        "routeCount": route_ids.len(),
        "routes": bus_route_details(&data, &route_ids, Some(query)),
    }))
}
