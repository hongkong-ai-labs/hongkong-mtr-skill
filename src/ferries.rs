use anyhow::{Result, anyhow, bail};
use chrono::{DateTime, Days, FixedOffset, NaiveTime, TimeZone};
use quick_xml::de::from_str;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::app::App;
use crate::config::Config;
use crate::http::HttpClient;

#[derive(Debug, Clone, Copy)]
pub struct FerryOperatorDef {
    pub id: &'static str,
    pub name_en: &'static str,
    pub name_tc: &'static str,
    pub website: &'static str,
    pub scope: &'static str,
    pub live_eta: bool,
    pub notes: &'static str,
}

pub const FERRY_OPERATORS: &[FerryOperatorDef] = &[
    FerryOperatorDef {
        id: "star-ferry",
        name_en: "The \"Star\" Ferry Company, Limited",
        name_tc: "天星小輪有限公司",
        website: "https://www.starferry.com.hk",
        scope: "local",
        live_eta: false,
        notes: "Franchised Victoria Harbour crossings: Central–Tsim Sha Tsui and Wan Chai–Tsim Sha Tsui. Frequencies and fares are published; no live ETA API.",
    },
    FerryOperatorDef {
        id: "sun-ferry",
        name_en: "Sun Ferry Services Company Limited",
        name_tc: "新渡輪服務有限公司",
        website: "https://www.sunferry.com.hk",
        scope: "local",
        live_eta: true,
        notes: "Outlying-island routes and North Point cross-harbour routes. Live ETA API: https://www.sunferry.com.hk/eta/?route=CODE.",
    },
    FerryOperatorDef {
        id: "hkkf",
        name_en: "Hong Kong & Kowloon Ferry Limited",
        name_tc: "香港九龍渡海小輪有限公司",
        website: "https://www.hkkf.com.hk",
        scope: "local",
        live_eta: true,
        notes: "Central–Lamma, Central–Peng Chau and Peng Chau–Hei Ling Chau. Live ETA API at hkkfeta.com/opendata/eta/{route_id}/{direction}.",
    },
    FerryOperatorDef {
        id: "discovery-bay",
        name_en: "Discovery Bay Transportation Services Limited",
        name_tc: "愉景灣交通服務有限公司",
        website: "https://www.dbcommunity.hk",
        scope: "local",
        live_eta: false,
        notes: "Central–Discovery Bay and Discovery Bay North services; also operates resident buses.",
    },
    FerryOperatorDef {
        id: "park-island",
        name_en: "Park Island Transport Company Limited",
        name_tc: "珀麗灣客運有限公司",
        website: "https://www.pitcl.com.hk",
        scope: "local",
        live_eta: false,
        notes: "Ma Wan–Central and Ma Wan–Tsuen Wan. Weekday sailings can be temporarily replaced by resident buses.",
    },
    FerryOperatorDef {
        id: "fortune-ferry",
        name_en: "Fortune Ferry Company Limited",
        name_tc: "富裕小輪有限公司",
        website: "https://www.fortuneferry.com.hk",
        scope: "local",
        live_eta: false,
        notes: "Central–Hung Hom, North Point–Kwun Tong–Kai Tak and Tuen Mun–Tung Chung–Sha Lo Wan–Tai O. The published ETA endpoint is unreliable, so this tool returns timetables/source links instead.",
    },
    FerryOperatorDef {
        id: "chuen-kee",
        name_en: "Chuen Kee Ferry Limited",
        name_tc: "全記渡有限公司",
        website: "https://www.ferry.com.hk",
        scope: "local",
        live_eta: false,
        notes: "Aberdeen–Sok Kwu Wan via Mo Tat.",
    },
    FerryOperatorDef {
        id: "coral-sea",
        name_en: "Coral Sea Ferry Service Company Limited",
        name_tc: "珊瑚海船務有限公司",
        website: "https://www.coralseaferryservice.com.hk",
        scope: "local",
        live_eta: false,
        notes: "Sai Wan Ho–Sam Ka Tsuen and Sai Wan Ho–Kwun Tong.",
    },
    FerryOperatorDef {
        id: "tsui-wah",
        name_en: "Tsui Wah Ferry Service (Hong Kong) Limited",
        name_tc: "翠華船務（香港）有限公司",
        website: "https://www.traway.com.hk",
        scope: "local",
        live_eta: false,
        notes: "Aberdeen–Pak Kok Tsuen–Yung Shue Wan and north-east New Territories / Sai Kung kaito routes including Tap Mun, Tung Ping Chau and Lai Chi Wo.",
    },
    FerryOperatorDef {
        id: "peng-chau-kaito",
        name_en: "Peng Chau Kai To Limited",
        name_tc: "坪洲街渡有限公司",
        website: "https://en.pengchaukaito.com",
        scope: "local",
        live_eta: false,
        notes: "Discovery Bay–Mui Wo (via Peng Chau), plus Peng Chau–Discovery Bay via Trappist Monastery.",
    },
    FerryOperatorDef {
        id: "hk-yaumati",
        name_en: "The Hongkong and Yaumati Ferry Company Limited",
        name_tc: "香港油蔴地小輪船有限公司",
        website: "https://www.hkf.com",
        scope: "dangerous-goods",
        live_eta: false,
        notes: "Dangerous-goods vehicle and emergency services only; not a normal passenger route.",
    },
    FerryOperatorDef {
        id: "water-taxi",
        name_en: "Hong Kong Water Taxi",
        name_tc: "香港水上的士",
        website: "https://www.hongkongwatertaxi.com.hk",
        scope: "water-taxi",
        live_eta: false,
        notes: "Water-taxi routes around Victoria Harbour and West Kowloon; bookable/limited-service rather than a conventional commuter ferry.",
    },
    FerryOperatorDef {
        id: "kaito",
        name_en: "Kaito / licensed small ferries",
        name_tc: "街渡",
        website: "https://www.td.gov.hk/en/transport_in_hong_kong/public_transport/ferries/kaito_services_map/index.html",
        scope: "kaito",
        live_eta: false,
        notes: "Licensed small ferries serving remote coastal settlements. Operators vary by route; no single live API.",
    },
    FerryOperatorDef {
        id: "turbojet",
        name_en: "TurboJET (Shun Tak–China Travel Ship Management)",
        name_tc: " TurboJET（信德中旅船務管理）",
        website: "https://www.turbojet.com.hk",
        scope: "cross-boundary",
        live_eta: false,
        notes: "Hong Kong–Macao and Pearl River Delta high-speed ferry services from the Hong Kong–Macao Ferry Terminal and China Ferry Terminal.",
    },
    FerryOperatorDef {
        id: "cotai-water-jet",
        name_en: "Cotai Water Jet (Chu Kong High-Speed Ferry)",
        name_tc: "金光飛航（珠江高速船）",
        website: "https://www.cotaiwaterjet.com",
        scope: "cross-boundary",
        live_eta: false,
        notes: "Hong Kong–Macao Taipa high-speed ferry services.",
    },
    FerryOperatorDef {
        id: "chu-kong",
        name_en: "Chu Kong Passenger Transport Company Limited",
        name_tc: "珠江客運有限公司",
        website: "https://www.cksp.com.hk",
        scope: "cross-boundary",
        live_eta: false,
        notes: "Hong Kong–Guangdong ports including Shekou, Zhongshan, Nansha, Shunde and Pazhou; SkyPier airport transits.",
    },
];

fn operator_def(id: &str) -> Option<&'static FerryOperatorDef> {
    FERRY_OPERATORS.iter().find(|operator| operator.id == id)
}

#[derive(Debug, Clone, Deserialize)]
struct FerryXml {
    #[serde(rename = "ROUTE", default)]
    routes: Vec<RawFerryRoute>,
}

#[derive(Debug, Clone, Deserialize)]
struct RawFerryRoute {
    #[serde(rename = "ROUTE_ID", default)]
    route_id: String,
    #[serde(rename = "DISTRICT", default)]
    district: String,
    #[serde(rename = "ROUTE_NAMEC", default)]
    name_tc: String,
    #[serde(rename = "ROUTE_NAMES", default)]
    name_sc: String,
    #[serde(rename = "ROUTE_NAMEE", default)]
    name_en: String,
    #[serde(rename = "SERVICE_MODE", default)]
    service_mode: String,
    #[serde(rename = "JOURNEY_TIME", default)]
    journey_time: String,
    #[serde(rename = "LOC_START_NAMEC", default)]
    origin_tc: String,
    #[serde(rename = "LOC_START_NAMEE", default)]
    origin_en: String,
    #[serde(rename = "LOC_END_NAMEC", default)]
    destination_tc: String,
    #[serde(rename = "LOC_END_NAMEE", default)]
    destination_en: String,
    #[serde(rename = "HYPERLINK_E", default)]
    hyperlink_en: String,
    #[serde(rename = "FULL_FARE", default)]
    full_fare: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FerryRoute {
    pub route_id: String,
    pub operator_id: String,
    pub operator_name: String,
    pub name_en: String,
    pub name_tc: String,
    pub name_sc: String,
    pub district: String,
    pub service_mode: String,
    pub journey_minutes: Option<i64>,
    pub origin_en: String,
    pub origin_tc: String,
    pub destination_en: String,
    pub destination_tc: String,
    pub full_fare: Option<f64>,
    pub source_url: Option<String>,
    pub live_eta: bool,
    pub eta_route: Option<String>,
    pub eta_direction: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FerryData {
    pub routes: Vec<FerryRoute>,
}

fn classify_operator(route_id: &str) -> &'static str {
    match route_id {
        "7030" | "7031" => "star-ferry",
        "7005" | "7006" | "7015" | "7019" | "7020" | "7000008" | "7000009" | "7000010" => {
            "sun-ferry"
        }
        "7007" | "7008" | "7009" | "7014" => "hkkf",
        "7010" | "7000023" => "discovery-bay",
        "7017" | "7018" => "park-island",
        "7021" | "7056" | "7025" | "7000003" | "7000004" | "7059" => "fortune-ferry",
        "7003" => "chuen-kee",
        "7022" | "7023" => "coral-sea",
        "7004" | "7016" | "7026" | "7027" | "7036" | "7055" | "7000006" | "7000014" | "7000025"
        | "7000029" | "7000030" => "tsui-wah",
        "7041" | "7032" | "7000013" | "7000016" => "peng-chau-kaito",
        "7001" | "7035" | "7051" | "7052" | "7053" | "7057" | "7058" | "7000005" | "7000020"
        | "7000027" | "7000028" | "7000031" => "kaito",
        "7000037" | "7000038" | "7000039" | "7000040" => "water-taxi",
        _ => "kaito",
    }
}

fn eta_mapping(route_id: &str) -> (bool, Option<String>, Option<String>) {
    match route_id {
        // HKKF route IDs from hkkfeta.com/opendata/route.
        "7008" => (true, Some("1".into()), Some("outbound".into())),
        "7009" => (true, Some("2".into()), Some("outbound".into())),
        "7007" => (true, Some("3".into()), Some("outbound".into())),
        "7014" => (true, Some("4".into()), Some("outbound".into())),
        // Sun Ferry route codes from its ETA API.
        "7005" => (true, Some("CECC".into()), None),
        "7006" => (true, Some("CEMW".into()), None),
        "7019" => (true, Some("NPHH".into()), None),
        "7020" => (true, Some("NPKC".into()), None),
        "7000003" => (false, None, None),
        _ => (false, None, None),
    }
}

pub fn build_ferry_data(xml: &str) -> Result<FerryData> {
    let parsed: FerryXml = from_str(xml.trim_start_matches('\u{feff}'))
        .map_err(|error| anyhow!("ferry route feed returned an unexpected document: {error}"))?;
    let mut routes: Vec<FerryRoute> = parsed
        .routes
        .into_iter()
        .filter(|route| !route.route_id.is_empty() && !route.name_en.is_empty())
        .map(|route| {
            let operator_id = classify_operator(&route.route_id).to_string();
            let operator_name = operator_def(&operator_id)
                .map(|operator| operator.name_en.to_string())
                .unwrap_or_else(|| operator_id.clone());
            let (live_eta, eta_route, eta_direction) = eta_mapping(&route.route_id);
            FerryRoute {
                route_id: route.route_id,
                operator_id,
                operator_name,
                name_en: route.name_en,
                name_tc: route.name_tc,
                name_sc: route.name_sc,
                district: route.district,
                service_mode: route.service_mode,
                journey_minutes: route.journey_time.parse::<i64>().ok(),
                origin_en: route.origin_en,
                origin_tc: route.origin_tc,
                destination_en: route.destination_en,
                destination_tc: route.destination_tc,
                full_fare: route.full_fare.parse::<f64>().ok(),
                source_url: (!route.hyperlink_en.is_empty()).then_some(route.hyperlink_en),
                live_eta,
                eta_route,
                eta_direction,
            }
        })
        .collect();
    routes.sort_by(|a, b| {
        a.operator_id
            .cmp(&b.operator_id)
            .then_with(|| a.name_en.cmp(&b.name_en))
            .then_with(|| a.route_id.cmp(&b.route_id))
    });
    Ok(FerryData { routes })
}

pub async fn load_ferries(http: &HttpClient, config: &Config) -> Result<FerryData> {
    let xml = http.get_text(&config.endpoints.ferry_routes, &[]).await?;
    build_ferry_data(&xml)
}

fn operator_routes(data: &FerryData, operator_id: &str) -> usize {
    data.routes
        .iter()
        .filter(|route| route.operator_id == operator_id)
        .count()
}

fn operator_json(data: &FerryData, operator: &FerryOperatorDef) -> Value {
    json!({
        "id": operator.id,
        "nameEn": operator.name_en,
        "nameTc": operator.name_tc,
        "website": operator.website,
        "scope": operator.scope,
        "liveEta": operator.live_eta,
        "routeCount": operator_routes(data, operator.id),
        "notes": operator.notes,
    })
}

fn route_matches(route: &FerryRoute, query: &str) -> bool {
    let lower = query.to_lowercase();
    route.route_id == query
        || route.name_en.to_lowercase().contains(&lower)
        || route.name_tc.contains(query)
        || route.name_sc.contains(query)
        || route.origin_en.to_lowercase().contains(&lower)
        || route.destination_en.to_lowercase().contains(&lower)
        || route.origin_tc.contains(query)
        || route.destination_tc.contains(query)
        || route.operator_name.to_lowercase().contains(&lower)
        || route.district.to_lowercase().contains(&lower)
}

pub async fn get_ferries(
    app: &App,
    query: Option<&str>,
    operator: Option<&str>,
    include_routes: bool,
) -> Result<Value> {
    let data = app.ferries().await?;
    let query = query.map(str::trim).filter(|query| !query.is_empty());
    let operator_filter = operator.map(str::trim).filter(|value| !value.is_empty());

    let selected_operator = match operator_filter {
        Some(value) => {
            let lower = value.to_lowercase();
            FERRY_OPERATORS.iter().find(|item| {
                item.id == lower
                    || item.name_en.to_lowercase().contains(&lower)
                    || item.name_tc.contains(value)
            })
        }
        None => None,
    };
    if operator_filter.is_some() && selected_operator.is_none() {
        bail!(
            "No ferry operator matched \"{}\". Known operators: {}",
            operator_filter.unwrap_or_default(),
            FERRY_OPERATORS
                .iter()
                .map(|item| item.id)
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    let mut routes: Vec<&FerryRoute> = data
        .routes
        .iter()
        .filter(|route| {
            selected_operator
                .map(|item| route.operator_id == item.id)
                .unwrap_or(true)
        })
        .filter(|route| {
            query
                .map(|query| route_matches(route, query))
                .unwrap_or(true)
        })
        .collect();

    let query_operators: Vec<&FerryOperatorDef> = match query {
        Some(value) => {
            let lower = value.to_lowercase();
            FERRY_OPERATORS
                .iter()
                .filter(|item| {
                    item.name_en.to_lowercase().contains(&lower)
                        || item.name_tc.contains(value)
                        || item.scope.to_lowercase().contains(&lower)
                        || item.notes.to_lowercase().contains(&lower)
                })
                .collect()
        }
        None => Vec::new(),
    };

    if query.is_some() && routes.is_empty() {
        if let Some(selected) = selected_operator {
            return Ok(json!({
                "count": 0,
                "totalRouteCount": data.routes.len(),
                "operators": [operator_json(&data, selected)],
                "routes": [],
                "note": "This operator has no local route rows in the Transport Department dataset. Cross-boundary, dangerous-goods and water-taxi services are listed for directory/reference; use the operator website for schedules and booking.",
                "sources": ["Transport Department ferry route/fare data", "Operator website"],
            }));
        }
        if !query_operators.is_empty() {
            return Ok(json!({
                "count": 0,
                "totalRouteCount": data.routes.len(),
                "operators": query_operators.iter().map(|item| operator_json(&data, item)).collect::<Vec<_>>(),
                "routes": [],
                "note": "No local route rows matched. These operators provide cross-boundary, kaito, water-taxi or specialised services; use their websites for schedules and booking.",
                "sources": ["Transport Department ferry route/fare data", "Operator websites"],
            }));
        }
        bail!(
            "No ferry route or operator matched \"{}\". Try a pier, island, route number or operator name.",
            query.unwrap_or_default()
        );
    }
    if !include_routes && query.is_none() && operator_filter.is_none() {
        routes.clear();
    }

    let operators: Vec<Value> = FERRY_OPERATORS
        .iter()
        .filter(|item| {
            selected_operator
                .map(|selected| item.id == selected.id)
                .unwrap_or(true)
        })
        .map(|item| operator_json(&data, item))
        .collect();

    Ok(json!({
        "count": routes.len(),
        "totalRouteCount": data.routes.len(),
        "operators": operators,
        "routes": routes,
        "sources": [
            "Transport Department franchised and licensed ferry route/fare data (ROUTE_FERRY.xml)",
            "Operator timetables and ETA APIs listed per operator",
            "Kaito and water-taxi services may be seasonal, bookable or demand-responsive; confirm with the operator",
        ],
    }))
}

#[derive(Debug, Clone, Deserialize)]
struct HkkfEtaResponse {
    #[serde(default)]
    data: Vec<HkkfEta>,
}

#[derive(Debug, Clone, Deserialize)]
struct HkkfEta {
    #[serde(default)]
    route_id: i64,
    #[serde(default)]
    direction: String,
    #[serde(default)]
    date: String,
    #[serde(default)]
    session_time: String,
    #[serde(rename = "ETA", default)]
    eta: String,
}

#[derive(Debug, Clone, Deserialize)]
struct SunEtaResponse {
    #[serde(rename = "generated_timestamp", default)]
    generated_timestamp: String,
    #[serde(default)]
    data: Option<Vec<SunEta>>,
}

#[derive(Debug, Clone, Deserialize)]
struct SunEta {
    #[serde(rename = "routecode", default)]
    route_code: String,
    #[serde(rename = "route_en", default)]
    route_en: String,
    #[serde(rename = "route_tc", default)]
    route_tc: String,
    #[serde(rename = "vesselcode", default)]
    vessel_code: Option<String>,
    #[serde(rename = "depart_time", default)]
    depart_time: String,
    #[serde(rename = "eta", default)]
    eta: Option<String>,
    #[serde(rename = "rmk_en", default)]
    remark_en: Option<String>,
    #[serde(rename = "rmk_tc", default)]
    remark_tc: Option<String>,
}

fn minutes_from_rfc3339(value: &str) -> Option<i64> {
    let eta = DateTime::parse_from_rfc3339(value).ok()?;
    let now = chrono::Utc::now();
    Some((eta.with_timezone(&chrono::Utc) - now).num_minutes().max(0))
}

fn minutes_until_hhmm(generated: &str, value: &str) -> Option<i64> {
    let generated = DateTime::parse_from_rfc3339(generated).ok()?;
    let time = NaiveTime::parse_from_str(value, "%H:%M").ok()?;
    let hkt = FixedOffset::east_opt(8 * 3600)?;
    let mut departure = hkt
        .from_local_datetime(&generated.date_naive().and_time(time))
        .single()?;
    if departure < generated {
        departure = departure.checked_add_days(Days::new(1))?;
    }
    Some((departure - generated).num_minutes().max(0))
}

pub async fn get_ferry_eta(
    app: &App,
    operator: &str,
    route_query: &str,
    direction: Option<&str>,
) -> Result<Value> {
    let data = app.ferries().await?;
    let operator = operator.trim().to_lowercase();
    let route = data
        .routes
        .iter()
        .find(|route| {
            route.operator_id == operator
                && (route.eta_route.as_deref() == Some(route_query.trim())
                    || route.route_id == route_query.trim()
                    || route.name_en.eq_ignore_ascii_case(route_query.trim()))
        })
        .or_else(|| {
            data.routes
                .iter()
                .find(|route| route.operator_id == operator && route_matches(route, route_query))
        })
        .ok_or_else(|| {
            anyhow!(
                "No {} route matched \"{}\". Use mtr_ferries to look up the operator route code.",
                operator,
                route_query
            )
        })?;

    if operator == "hkkf" {
        let route_id = route
            .eta_route
            .as_deref()
            .unwrap_or(route.route_id.as_str());
        let direction = direction.unwrap_or("outbound");
        let url = format!(
            "{}/{}/{}",
            app.config.endpoints.hkkf_eta_base, route_id, direction
        );
        let raw: HkkfEtaResponse = app.http.get_json(&url, &[]).await?;
        let etas: Vec<Value> = raw
            .data
            .iter()
            .map(|eta| {
                json!({
                    "route": route.route_id,
                    "operatorRouteId": eta.route_id,
                    "direction": eta.direction,
                    "date": eta.date,
                    "departTime": eta.session_time,
                    "eta": eta.eta,
                    "minutesAway": minutes_from_rfc3339(&eta.eta),
                })
            })
            .collect();
        return Ok(json!({
            "operator": "hkkf",
            "operatorName": "Hong Kong & Kowloon Ferry Limited",
            "route": route,
            "live": true,
            "source": url,
            "etas": etas,
        }));
    }

    if operator == "sun-ferry" {
        let code = route.eta_route.as_deref().ok_or_else(|| {
            anyhow!(
                "Sun Ferry live ETA is only wired for its core routes; route {} has no ETA code.",
                route.route_id
            )
        })?;
        let url = format!(
            "{}/?route={}",
            app.config.endpoints.sun_ferry_eta.trim_end_matches('/'),
            code
        );
        let raw: SunEtaResponse = app.http.get_json(&url, &[]).await?;
        let etas: Vec<Value> = raw
            .data
            .unwrap_or_default()
            .iter()
            .map(|eta| {
                json!({
                    "routeCode": eta.route_code,
                    "routeEn": eta.route_en,
                    "routeTc": eta.route_tc,
                    "vesselCode": eta.vessel_code,
                    "departTime": eta.depart_time,
                    "eta": eta.eta,
                    "minutesAway": eta
                        .eta
                        .as_deref()
                        .and_then(|value| minutes_until_hhmm(&raw.generated_timestamp, value))
                        .or_else(|| minutes_until_hhmm(&raw.generated_timestamp, &eta.depart_time)),
                    "remarkEn": eta.remark_en,
                    "remarkTc": eta.remark_tc,
                })
            })
            .collect();
        return Ok(json!({
            "operator": "sun-ferry",
            "operatorName": "Sun Ferry Services Company Limited",
            "route": route,
            "live": true,
            "generatedAt": raw.generated_timestamp,
            "source": url,
            "etas": etas,
        }));
    }

    let operator_def = operator_def(&operator)
        .ok_or_else(|| anyhow!("Unknown ferry operator \"{}\".", operator))?;
    Ok(json!({
        "operator": operator,
        "operatorName": operator_def.name_en,
        "route": route,
        "live": false,
        "message": "This operator does not publish a public live ETA API. Use its timetable and the route source link.",
        "source": operator_def.website,
        "timetableSource": route.source_url,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_known_operators() {
        let xml = r#"<dataroot>
        <ROUTE><ROUTE_ID>7030</ROUTE_ID><DISTRICT>INNER</DISTRICT><ROUTE_NAMEE>Tsim Sha Tsui - Central</ROUTE_NAMEE><FULL_FARE>6.5</FULL_FARE><JOURNEY_TIME>8</JOURNEY_TIME></ROUTE>
        <ROUTE><ROUTE_ID>7008</ROUTE_ID><DISTRICT>OUTLYING</DISTRICT><ROUTE_NAMEE>Central - Sok Kwu Wan</ROUTE_NAMEE><FULL_FARE>43.5</FULL_FARE><JOURNEY_TIME>35</JOURNEY_TIME></ROUTE>
        </dataroot>"#;
        let data = build_ferry_data(xml).unwrap();
        assert_eq!(data.routes.len(), 2);
        let star = data
            .routes
            .iter()
            .find(|route| route.route_id == "7030")
            .unwrap();
        let hkkf = data
            .routes
            .iter()
            .find(|route| route.route_id == "7008")
            .unwrap();
        assert_eq!(star.operator_id, "star-ferry");
        assert_eq!(hkkf.operator_id, "hkkf");
        assert!(hkkf.live_eta);
        assert_eq!(hkkf.journey_minutes, Some(35));
    }

    #[test]
    fn minutes_until_hhmm_handles_next_day() {
        let value = minutes_until_hhmm("2026-11-21T23:51:00+08:00", "00:04");
        assert_eq!(value, Some(13));
    }
}
