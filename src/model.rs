use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RailStation {
    pub id: i64,
    pub code: String,
    pub name_en: String,
    pub name_tc: String,
    pub name_sc: String,
    pub line_ids: Vec<i64>,
    pub lat: Option<f64>,
    pub lng: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RailLine {
    pub id: i64,
    pub code: String,
    pub name_en: String,
    pub name_tc: String,
    pub color: String,
    pub station_ids: Vec<i64>,
    pub has_real_time: bool,
}

#[derive(Debug, Clone)]
pub struct Network {
    pub stations: Vec<RailStation>,
    pub lines: Vec<RailLine>,
    pub station_by_id: HashMap<i64, usize>,
    pub station_by_code: HashMap<String, usize>,
    pub line_by_id: HashMap<i64, usize>,
    pub line_by_code: HashMap<String, usize>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StationSummary {
    pub id: i64,
    pub code: String,
    pub name_en: String,
    pub name_tc: String,
    pub lines: Vec<String>,
    pub coordinates: Option<Coordinates>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Coordinates {
    pub latitude: f64,
    pub longitude: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LineRef {
    pub code: String,
    pub name_en: String,
    pub name_tc: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StationRecord {
    #[serde(flatten)]
    pub summary: StationSummary,
    pub line_names: Vec<LineRef>,
    pub interchange: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AlertSeverity {
    Disruption,
    Delay,
    Notice,
    Record,
}

impl AlertSeverity {
    pub fn rank(self) -> u8 {
        match self {
            Self::Disruption => 0,
            Self::Delay => 1,
            Self::Notice => 2,
            Self::Record => 3,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlertSummary {
    pub id: String,
    pub title: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub tsi_type: String,
    pub active: bool,
    pub severity: AlertSeverity,
    pub lines: Vec<String>,
    pub line_names: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Alert {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub tsi_type: String,
    pub active: bool,
    pub severity: AlertSeverity,
    pub lines: Vec<String>,
    pub line_names: Vec<String>,
    pub title: String,
    pub title_tc: String,
    pub content: String,
    pub content_tc: String,
    pub cause: String,
    pub cause_tc: String,
    pub issued_time: Option<String>,
    pub details_url: Option<String>,
    pub has_map: bool,
    pub has_shuttle_bus: bool,
    pub has_alternative_transport: bool,
}

impl Alert {
    pub fn summary(&self) -> AlertSummary {
        AlertSummary {
            id: self.id.clone(),
            title: self.title.clone(),
            kind: self.kind.clone(),
            tsi_type: self.tsi_type.clone(),
            active: self.active,
            severity: self.severity,
            lines: self.lines.clone(),
            line_names: self.line_names.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct AlertCounts {
    pub active: usize,
    pub disruption: usize,
    pub delay: usize,
    pub notice: usize,
    pub record: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlertsResult {
    pub updated_at: String,
    pub source: String,
    pub count: usize,
    pub network_wide: usize,
    pub counts: AlertCounts,
    pub alerts: Vec<Alert>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LineStatus {
    pub code: String,
    pub name_en: String,
    pub name_tc: String,
    pub color: String,
    pub status: String,
    pub status_label: String,
    pub status_label_tc: String,
    pub status_description: String,
    pub details_url: Option<String>,
    pub updated_at: Option<String>,
    pub alerts: Vec<AlertSummary>,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct StatusSummary {
    pub normal_lines: usize,
    pub delayed_lines: Vec<String>,
    pub disrupted_lines: Vec<String>,
    pub typhoon_lines: Vec<String>,
    pub special_lines: Vec<String>,
    pub non_service_lines: Vec<String>,
    pub notice_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusResult {
    pub updated_at: Option<String>,
    pub updated_at_hkt: Option<String>,
    pub refresh_seconds: Option<i64>,
    pub overall: String,
    pub remark: String,
    pub remark_tc: String,
    pub summary: StatusSummary,
    pub lines: Vec<LineStatus>,
    pub alerts: Vec<AlertSummary>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NextTrain {
    pub seq: i64,
    pub destination_code: String,
    pub destination: String,
    pub platform: Option<String>,
    pub time: String,
    pub minutes_away: Option<i64>,
    pub valid: bool,
    pub source: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NextTrainDirection {
    pub direction: String,
    pub trains: Vec<NextTrain>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StationNextTrains {
    pub station: StationRef,
    pub line: LineRefFull,
    pub delayed: bool,
    pub message: Option<String>,
    pub directions: Vec<NextTrainDirection>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StationRef {
    pub id: i64,
    pub code: String,
    pub name_en: String,
    pub name_tc: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LineRefFull {
    pub code: String,
    pub name_en: String,
    pub name_tc: String,
    pub color: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrainWarning {
    pub line: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NextTrainResult {
    pub station: StationRef,
    pub delayed: bool,
    pub warnings: Vec<TrainWarning>,
    pub lines: Vec<StationNextTrains>,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct JourneyFares {
    pub octopus_adult: Option<f64>,
    pub octopus_child: Option<f64>,
    pub octopus_elderly: Option<f64>,
    pub octopus_student: Option<f64>,
    pub octopus_joy_you: Option<f64>,
    pub single_adult: Option<f64>,
    pub single_child: Option<f64>,
    pub single_elderly: Option<f64>,
    pub first_class_octopus_adult: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JourneyLeg {
    #[serde(rename = "type")]
    pub kind: String,
    pub line_code: Option<String>,
    pub line_name: Option<String>,
    pub line_color: Option<String>,
    pub from: String,
    pub from_id: i64,
    pub to: String,
    pub to_id: i64,
    pub board_platform: Option<String>,
    pub towards: Option<String>,
    pub start_minute: i64,
    pub end_minute: i64,
    pub duration_minutes: i64,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JourneyPathStep {
    pub station_id: i64,
    pub station: String,
    pub line_code: Option<String>,
    pub line_name: Option<String>,
    pub link_type: String,
    pub minute: i64,
    pub platform: Option<String>,
    pub towards: Option<String>,
    pub instruction: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JourneyRoute {
    pub route_name: String,
    pub time_minutes: i64,
    pub interchanges: i64,
    pub walking_minutes: i64,
    pub fares: JourneyFares,
    pub legs: Vec<JourneyLeg>,
    pub path: Vec<JourneyPathStep>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrainWindow {
    pub time: String,
    pub interchange: Option<serde_json::Value>,
    pub links: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JourneyResult {
    pub from: StationSummary,
    pub to: StationSummary,
    pub first_train: Option<TrainWindow>,
    pub last_train: Option<TrainWindow>,
    pub note: Option<String>,
    pub routes: Vec<JourneyRoute>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LrStop {
    pub id: i64,
    pub code: String,
    pub name_en: String,
    pub name_tc: String,
}

#[derive(Debug, Clone)]
pub struct LrRoute {
    pub route: String,
    pub direction: String,
    pub stops: Vec<LrStop>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LrPlatformRoute {
    pub route: String,
    pub destination: String,
    pub destination_tc: String,
    pub platform: i64,
    pub arrival_departure: String,
    pub minutes_away: Option<i64>,
    pub time_text: String,
    pub time_text_tc: String,
    pub special: bool,
    pub train_length: i64,
    pub remark: Option<String>,
    pub remark_tc: Option<String>,
    pub additional_info: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LrPlatform {
    pub platform_id: i64,
    pub routes: Vec<LrPlatformRoute>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LrNextResult {
    pub stop: LrStop,
    pub system_time: Option<String>,
    pub platforms: Vec<LrPlatform>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BusRouteVariant {
    pub route_id: String,
    pub name_en: String,
    pub name_tc: String,
    pub circular: bool,
    pub line_up: String,
    pub line_down: String,
    pub reference_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BusStop {
    pub route_id: String,
    pub direction: String,
    pub sequence: i64,
    pub id: String,
    pub name_en: String,
    pub name_tc: String,
    pub lat: Option<f64>,
    pub lng: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BusFare {
    pub route_id: String,
    pub octopus: BusFareSet,
    pub single: BusFareSet,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BusFareSet {
    pub adult: Option<f64>,
    pub child: Option<f64>,
    pub elderly: Option<f64>,
    pub joy_you: Option<f64>,
    pub pwd: Option<f64>,
    pub student: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct BusData {
    pub routes: Vec<BusRouteVariant>,
    pub stops: Vec<BusStop>,
    pub routes_by_id: HashMap<String, Vec<BusRouteVariant>>,
    pub stops_by_route: HashMap<String, Vec<BusStop>>,
    pub fares: HashMap<String, BusFare>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BusArrival {
    pub bus_id: String,
    pub minutes_away: Option<i64>,
    pub departure_text: String,
    pub delayed: bool,
    pub scheduled: bool,
    pub line_ref: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BusEta {
    pub route: String,
    pub direction: String,
    pub direction_code: String,
    pub stop_id: String,
    pub stop_name_en: Option<String>,
    pub stop_name_tc: Option<String>,
    pub sequence: Option<i64>,
    pub suspended: bool,
    pub buses: Vec<BusArrival>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BusServiceStatus {
    pub route_id: String,
    pub variant: String,
    pub status: Option<String>,
    pub status_colour: Option<String>,
    pub status_title: Option<String>,
    pub status_remark: Option<String>,
    pub updated_at: Option<String>,
    pub refresh_seconds: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BusNextResult {
    pub route: String,
    pub service_status: Vec<BusServiceStatus>,
    pub suspended_stop_count: usize,
    pub variants: Vec<BusRouteVariant>,
    pub fares: HashMap<String, Option<BusFare>>,
    pub stop_count: usize,
    pub stops: Vec<BusEta>,
    pub remark: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FacilityItem {
    pub key: String,
    pub exit_en: String,
    pub exit_tc: String,
    pub location: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Facility {
    pub name: String,
    pub name_tc: String,
    pub items: Vec<FacilityItem>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FacilityGroup {
    pub category: String,
    pub category_tc: String,
    pub facilities: Vec<Facility>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StationFacilities {
    pub station: StationRef,
    pub categories: Vec<FacilityGroup>,
    pub note: String,
    pub lines: Vec<String>,
    pub coordinates: Option<Coordinates>,
}

// Raw upstream types used by the trip planner and live feeds.

#[derive(Debug, Clone, Deserialize)]
pub struct RawStationResponse {
    #[serde(rename = "errorCode", default)]
    pub error_code: Option<String>,
    #[serde(rename = "errorMsg", default)]
    pub error_msg: Option<String>,
    #[serde(default)]
    pub lines: Vec<RawStationLine>,
    #[serde(default)]
    pub stations: Vec<RawStation>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawStationLine {
    #[serde(rename = "ID")]
    pub id: i64,
    #[serde(default)]
    pub alias: String,
    #[serde(rename = "nameEN", default)]
    pub name_en: String,
    #[serde(rename = "nameTC", default)]
    pub name_tc: String,
    #[serde(default)]
    pub color: String,
    #[serde(rename = "stationIDs", default)]
    pub station_ids: Vec<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawStation {
    #[serde(rename = "ID")]
    pub id: i64,
    #[serde(default)]
    pub alias: String,
    #[serde(rename = "nameEN", default)]
    pub name_en: String,
    #[serde(rename = "nameTC", default)]
    pub name_tc: String,
    #[serde(rename = "nameSC", default)]
    pub name_sc: Option<String>,
    #[serde(rename = "lineIDs", default)]
    pub line_ids: Vec<i64>,
    #[serde(default)]
    pub coordinate: Option<String>,
}
