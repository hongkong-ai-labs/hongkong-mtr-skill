use anyhow::{Result, anyhow, bail};
use serde::Deserialize;

use crate::app::App;
use crate::http::HttpBackend;
use crate::model::{
    JourneyFares, JourneyLeg, JourneyPathStep, JourneyResult, JourneyRoute, Network,
    StationSummary, TrainWindow,
};
use crate::network::{line_for_id, resolve_station, station_name, station_summary, towards_label};
use crate::text::html_to_text;

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct RawPath {
    #[serde(rename = "ID")]
    id: i64,
    #[serde(rename = "lineID", default)]
    line_id: i64,
    #[serde(rename = "linkType", default)]
    link_type: String,
    #[serde(default)]
    time: String,
    #[serde(default)]
    platform: Option<String>,
    #[serde(default)]
    towards: Option<String>,
    #[serde(rename = "linkText", default)]
    link_text: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct RawFareSet {
    #[serde(rename = "octopusAdult", default)]
    octopus_adult: Option<String>,
    #[serde(rename = "octopusConcession", default)]
    octopus_concession: Option<String>,
    #[serde(rename = "octopusConcessionChild", default)]
    octopus_concession_child: Option<String>,
    #[serde(rename = "octopusConcessionElderly", default)]
    octopus_concession_elderly: Option<String>,
    #[serde(rename = "octopusStudent", default)]
    octopus_student: Option<String>,
    #[serde(rename = "octopusJoyYouSixty", default)]
    octopus_joy_you_sixty: Option<String>,
    #[serde(rename = "sjAdult", default)]
    single_adult: Option<String>,
    #[serde(rename = "sjConcession", default)]
    single_concession: Option<String>,
    #[serde(rename = "sjConcessionChild", default)]
    single_concession_child: Option<String>,
    #[serde(rename = "sjConcessionElderly", default)]
    single_concession_elderly: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct RawFares {
    #[serde(rename = "AEL", default)]
    ael: Option<RawFareSet>,
    #[serde(rename = "standardClass", default)]
    standard_class: Option<RawFareSet>,
    #[serde(rename = "firstClass", default)]
    first_class: Option<RawFareSet>,
}

#[derive(Debug, Clone, Deserialize)]
struct RawRoute {
    #[serde(default)]
    time: f64,
    #[serde(rename = "routeName", default)]
    route_name: String,
    #[serde(rename = "interchangeStationsNo", default)]
    interchange_stations_no: f64,
    #[serde(rename = "walkingTime", default)]
    walking_time: f64,
    #[serde(default)]
    fares: Option<RawFares>,
    #[serde(default)]
    path: Vec<RawPath>,
}

#[derive(Debug, Clone, Deserialize)]
struct RawTrainWindow {
    #[serde(default)]
    time: String,
    #[serde(default)]
    interchange: Option<serde_json::Value>,
    #[serde(default)]
    links: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct RawJourney {
    #[serde(rename = "errorCode", default)]
    error_code: serde_json::Value,
    #[serde(rename = "errorMsg", default)]
    error_msg: String,
    #[serde(rename = "firstTrain", default)]
    first_train: Option<RawTrainWindow>,
    #[serde(rename = "lastTrain", default)]
    last_train: Option<RawTrainWindow>,
    #[serde(rename = "firstLastTrainRemark", default)]
    first_last_train_remark: Option<String>,
    #[serde(default)]
    routes: Vec<RawRoute>,
}

fn to_number(value: Option<&String>) -> Option<f64> {
    let value = value?.trim();
    if value.is_empty() || value == "-" {
        return None;
    }
    value.parse::<f64>().ok()
}

fn normalize_fares(raw: Option<&RawFares>) -> JourneyFares {
    let Some(raw) = raw else {
        return JourneyFares::default();
    };
    let standard = raw.standard_class.as_ref().or(raw.ael.as_ref());
    JourneyFares {
        octopus_adult: to_number(standard.and_then(|fare| fare.octopus_adult.as_ref())),
        octopus_child: to_number(standard.and_then(|fare| fare.octopus_concession_child.as_ref()))
            .or_else(|| to_number(standard.and_then(|fare| fare.octopus_concession.as_ref()))),
        octopus_elderly: to_number(
            standard.and_then(|fare| fare.octopus_concession_elderly.as_ref()),
        )
        .or_else(|| to_number(standard.and_then(|fare| fare.octopus_concession.as_ref()))),
        octopus_student: to_number(standard.and_then(|fare| fare.octopus_student.as_ref())),
        octopus_joy_you: to_number(standard.and_then(|fare| fare.octopus_joy_you_sixty.as_ref())),
        single_adult: to_number(standard.and_then(|fare| fare.single_adult.as_ref())),
        single_child: to_number(standard.and_then(|fare| fare.single_concession_child.as_ref()))
            .or_else(|| to_number(standard.and_then(|fare| fare.single_concession.as_ref()))),
        single_elderly: to_number(
            standard.and_then(|fare| fare.single_concession_elderly.as_ref()),
        )
        .or_else(|| to_number(standard.and_then(|fare| fare.single_concession.as_ref()))),
        first_class_octopus_adult: to_number(
            raw.first_class
                .as_ref()
                .and_then(|fare| fare.octopus_adult.as_ref()),
        ),
    }
}

fn line_meta(network: &Network, line_id: i64) -> (Option<String>, Option<String>, Option<String>) {
    match line_for_id(network, line_id) {
        Some(line) => (
            Some(line.code.clone()),
            Some(line.name_en.clone()),
            Some(line.color.clone()),
        ),
        None => (None, None, None),
    }
}

struct LegBuilder<'a> {
    network: &'a Network,
    legs: Vec<JourneyLeg>,
    current: Option<JourneyLeg>,
    pending_walk: Option<JourneyLeg>,
}

impl<'a> LegBuilder<'a> {
    fn new(network: &'a Network) -> Self {
        Self {
            network,
            legs: Vec::new(),
            current: None,
            pending_walk: None,
        }
    }

    fn start_ride(&mut self, step: &RawPath, minute: i64) {
        let (line_code, line_name, line_color) = line_meta(self.network, step.line_id);
        self.current = Some(JourneyLeg {
            kind: "ride".to_string(),
            line_code,
            line_name,
            line_color,
            from: station_name(self.network, step.id),
            from_id: step.id,
            to: station_name(self.network, step.id),
            to_id: step.id,
            board_platform: step.platform.clone(),
            towards: towards_label(self.network, step.towards.as_deref()),
            start_minute: minute,
            end_minute: minute,
            duration_minutes: 0,
            note: {
                let text = html_to_text(step.link_text.as_deref());
                (!text.is_empty()).then_some(text)
            },
        });
    }

    fn close(&mut self, station_id: i64, minute: i64) {
        let Some(mut current) = self.current.take() else {
            return;
        };
        current.to = station_name(self.network, station_id);
        current.to_id = station_id;
        current.end_minute = minute;
        current.duration_minutes = (minute - current.start_minute).max(0);
        self.legs.push(current);
    }

    fn flush_walk(&mut self, station_id: i64, minute: i64) {
        let Some(mut walk) = self.pending_walk.take() else {
            return;
        };
        walk.to = station_name(self.network, station_id);
        walk.to_id = station_id;
        walk.end_minute = minute;
        walk.duration_minutes = (minute - walk.start_minute).max(0);
        self.legs.push(walk);
    }

    fn finish(mut self, path: &[RawPath]) -> Vec<JourneyLeg> {
        let last = path.last();
        self.close(
            last.map(|step| step.id).unwrap_or(0),
            last.and_then(|step| step.time.parse::<i64>().ok())
                .unwrap_or(0),
        );
        if let Some(walk) = self.pending_walk.take() {
            self.flush_walk(walk.to_id, walk.end_minute);
        }
        self.legs
            .into_iter()
            .filter(|leg| {
                leg.duration_minutes > 0 || leg.from_id != leg.to_id || leg.kind == "ride"
            })
            .collect()
    }
}

/// Convert MTR's flat path into readable legs.
pub(crate) fn build_legs(network: &Network, path: &[RawPath]) -> Vec<JourneyLeg> {
    let mut builder = LegBuilder::new(network);

    for (index, step) in path.iter().enumerate() {
        let minute = step.time.parse::<i64>().unwrap_or(0);
        let next = path.get(index + 1);

        match step.link_type.as_str() {
            "RIDE" => {
                if builder.current.is_none() {
                    builder.start_ride(step, minute);
                }
                if let Some(current) = builder.current.as_mut() {
                    current.to = station_name(network, step.id);
                    current.to_id = step.id;
                    current.end_minute = minute;
                    current.duration_minutes = (minute - current.start_minute).max(0);
                }
            }
            "INTERCHANGE" => {
                builder.close(step.id, minute);
                builder.flush_walk(step.id, minute);
                builder.start_ride(step, minute);
            }
            "WALKINTERCHANGE" => {
                builder.close(step.id, minute);
                if builder.pending_walk.is_some() {
                    builder.flush_walk(step.id, minute);
                }
                builder.pending_walk = Some(JourneyLeg {
                    kind: "walk".to_string(),
                    line_code: None,
                    line_name: None,
                    line_color: None,
                    from: station_name(network, step.id),
                    from_id: step.id,
                    to: next
                        .map(|next| station_name(network, next.id))
                        .unwrap_or_else(|| station_name(network, step.id)),
                    to_id: next.map(|next| next.id).unwrap_or(step.id),
                    board_platform: None,
                    towards: None,
                    start_minute: minute,
                    end_minute: minute,
                    duration_minutes: 0,
                    note: {
                        let text = html_to_text(step.link_text.as_deref());
                        (!text.is_empty()).then_some(text)
                    },
                });
            }
            _ => {
                builder.close(step.id, minute);
                builder.flush_walk(step.id, minute);
            }
        }
    }

    builder.finish(path)
}

fn enrich_path(network: &Network, path: &[RawPath]) -> Vec<JourneyPathStep> {
    path.iter()
        .map(|step| {
            let (line_code, line_name, _) = line_meta(network, step.line_id);
            let instruction = html_to_text(step.link_text.as_deref());
            JourneyPathStep {
                station_id: step.id,
                station: station_name(network, step.id),
                line_code,
                line_name,
                link_type: step.link_type.clone(),
                minute: step.time.parse::<i64>().unwrap_or(0),
                platform: step.platform.clone(),
                towards: towards_label(network, step.towards.as_deref()),
                instruction: (!instruction.is_empty()).then_some(instruction),
            }
        })
        .collect()
}

fn train_window(raw: Option<RawTrainWindow>) -> Option<TrainWindow> {
    raw.map(|window| TrainWindow {
        time: window.time,
        interchange: window.interchange,
        links: window.links,
    })
}

pub async fn plan_journey(
    app: &App<impl HttpBackend>,
    from_query: &str,
    to_query: &str,
    max_routes: Option<u8>,
) -> Result<JourneyResult> {
    let network = app.network().await?;
    let from = resolve_station(&network, from_query)?;
    let to = resolve_station(&network, to_query)?;
    if from.id == to.id {
        bail!("Origin and destination are the same station.");
    }

    let url = format!(
        "{}?o={}&d={}",
        app.config.endpoints.journey_api, from.id, to.id
    );
    let raw: RawJourney = app
        .http
        .get_json(&url, &[("api-key", app.config.trip_planner_key.as_str())])
        .await?;

    let ok = raw.error_code.as_str() == Some("0") || raw.error_code.as_i64() == Some(0);
    if !ok {
        let message = raw.error_msg.trim();
        return Err(anyhow!(
            "MTR trip planner: {}",
            if message.is_empty() {
                format!("error {}", raw.error_code)
            } else {
                message.to_string()
            }
        ));
    }

    let requested = max_routes.unwrap_or(3).clamp(1, 6) as usize;
    let routes: Vec<JourneyRoute> = raw
        .routes
        .iter()
        .take(requested)
        .map(|route| JourneyRoute {
            route_name: route.route_name.clone(),
            time_minutes: route.time as i64,
            interchanges: route.interchange_stations_no as i64,
            walking_minutes: route.walking_time as i64,
            fares: normalize_fares(route.fares.as_ref()),
            legs: build_legs(&network, &route.path),
            path: enrich_path(&network, &route.path),
        })
        .collect();

    let from_summary: StationSummary = station_summary(&network, from);
    let to_summary: StationSummary = station_summary(&network, to);
    Ok(JourneyResult {
        from: from_summary,
        to: to_summary,
        first_train: train_window(raw.first_train),
        last_train: train_window(raw.last_train),
        note: raw.first_last_train_remark.or_else(|| {
            Some(
                "Estimated journey time is based on weekday morning traffic and is for reference only."
                    .to_string(),
            )
        }),
        routes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{RailLine, RailStation};

    fn network() -> Network {
        let stations = vec![
            RailStation {
                id: 1,
                code: "ADM".into(),
                name_en: "Admiralty".into(),
                name_tc: "金鐘".into(),
                name_sc: "金钟".into(),
                line_ids: vec![11],
                lat: None,
                lng: None,
            },
            RailStation {
                id: 2,
                code: "CEN".into(),
                name_en: "Central".into(),
                name_tc: "中環".into(),
                name_sc: "中环".into(),
                line_ids: vec![11],
                lat: None,
                lng: None,
            },
            RailStation {
                id: 3,
                code: "HOK".into(),
                name_en: "Hong Kong".into(),
                name_tc: "香港".into(),
                name_sc: "香港".into(),
                line_ids: vec![14],
                lat: None,
                lng: None,
            },
            RailStation {
                id: 4,
                code: "TUC".into(),
                name_en: "Tung Chung".into(),
                name_tc: "東涌".into(),
                name_sc: "东涌".into(),
                line_ids: vec![14],
                lat: None,
                lng: None,
            },
        ];
        let lines = vec![
            RailLine {
                id: 11,
                code: "TWL".into(),
                name_en: "Tsuen Wan Line".into(),
                name_tc: "荃灣綫".into(),
                color: "#FF0000".into(),
                station_ids: vec![1, 2],
                has_real_time: true,
            },
            RailLine {
                id: 14,
                code: "TCL".into(),
                name_en: "Tung Chung Line".into(),
                name_tc: "東涌綫".into(),
                color: "#FE7F1D".into(),
                station_ids: vec![3, 4],
                has_real_time: true,
            },
        ];
        Network {
            station_by_id: stations
                .iter()
                .enumerate()
                .map(|(index, station)| (station.id, index))
                .collect(),
            station_by_code: stations
                .iter()
                .enumerate()
                .map(|(index, station)| (station.code.clone(), index))
                .collect(),
            line_by_id: lines
                .iter()
                .enumerate()
                .map(|(index, line)| (line.id, index))
                .collect(),
            line_by_code: lines
                .iter()
                .enumerate()
                .map(|(index, line)| (line.code.clone(), index))
                .collect(),
            stations,
            lines: lines.clone(),
        }
    }

    #[test]
    fn journey_legs_group_ride_walk_and_interchange() {
        let path = vec![
            RawPath {
                id: 1,
                line_id: 11,
                link_type: "RIDE".into(),
                time: "0".into(),
                platform: Some("2".into()),
                towards: Some("2".into()),
                link_text: Some("Board at Admiralty Platform 2 towards Central".into()),
            },
            RawPath {
                id: 2,
                line_id: 11,
                link_type: "WALKINTERCHANGE".into(),
                time: "3".into(),
                platform: Some("4".into()),
                towards: Some("4".into()),
                link_text: Some("Walk to Hong Kong Station".into()),
            },
            RawPath {
                id: 3,
                line_id: 14,
                link_type: "INTERCHANGE".into(),
                time: "8".into(),
                platform: Some("3 / 4".into()),
                towards: Some("4".into()),
                link_text: Some(
                    "Interchange at Hong Kong Platform 3 or 4 towards Tung Chung".into(),
                ),
            },
            RawPath {
                id: 4,
                line_id: 14,
                link_type: "END".into(),
                time: "20".into(),
                platform: None,
                towards: None,
                link_text: None,
            },
        ];
        let legs = build_legs(&network(), &path);
        assert_eq!(legs.len(), 3);
        let summary: Vec<(&str, &str, &str, i64)> = legs
            .iter()
            .map(|leg| {
                (
                    leg.kind.as_str(),
                    leg.from.as_str(),
                    leg.to.as_str(),
                    leg.duration_minutes,
                )
            })
            .collect();
        assert_eq!(
            summary,
            vec![
                ("ride", "Admiralty", "Central", 3),
                ("walk", "Central", "Hong Kong", 5),
                ("ride", "Hong Kong", "Tung Chung", 12)
            ]
        );
        assert_eq!(legs[2].line_code.as_deref(), Some("TCL"));
        assert!(
            legs[2]
                .note
                .as_deref()
                .unwrap_or("")
                .contains("Interchange at Hong Kong")
        );
    }
}
