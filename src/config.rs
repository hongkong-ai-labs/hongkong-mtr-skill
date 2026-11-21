use std::env;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct Endpoints {
    pub next_train: String,
    pub light_rail: String,
    pub bus: String,
    pub status: String,
    pub legend: String,
    pub alerts: String,
    pub late_cert: String,
    pub line_config: String,
    pub stations_api: String,
    pub journey_api: String,
    pub ferry_routes: String,
    pub hkkf_eta_base: String,
    pub sun_ferry_eta: String,
}

#[derive(Debug, Clone)]
pub struct Datasets {
    pub lines_and_stations: String,
    pub lines_fares: String,
    pub light_rail_routes_and_stops: String,
    pub light_rail_fares: String,
    pub bus_routes: String,
    pub bus_stops: String,
    pub bus_fares: String,
    pub barrier_free: String,
    pub barrier_free_categories: String,
}

#[derive(Debug, Clone)]
pub struct CacheConfig {
    pub static_data: Duration,
    pub status: Duration,
    pub alerts: Duration,
    pub journey: Duration,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub user_agent: String,
    pub endpoints: Endpoints,
    pub datasets: Datasets,
    pub trip_planner_key: String,
    pub timeout: Duration,
    pub cache: CacheConfig,
}

fn env_or(key: &str, fallback: &str) -> String {
    env::var(key).unwrap_or_else(|_| fallback.to_string())
}

impl Config {
    pub fn from_env() -> Self {
        let timeout_ms = env::var("MTR_HTTP_TIMEOUT_MS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(12_000);

        Self {
            user_agent: "mtr-hk-mcp/1.0 (+https://github.com/; contact: local-user)".to_string(),
            endpoints: Endpoints {
                next_train: env_or(
                    "MTR_NEXT_TRAIN_URL",
                    "https://rt.data.gov.hk/v1/transport/mtr/getSchedule.php",
                ),
                light_rail: env_or(
                    "MTR_LIGHT_RAIL_URL",
                    "https://rt.data.gov.hk/v1/transport/mtr/lrt/getSchedule",
                ),
                bus: env_or(
                    "MTR_BUS_URL",
                    "https://rt.data.gov.hk/v1/transport/mtr/bus/getSchedule",
                ),
                status: env_or(
                    "MTR_STATUS_URL",
                    "https://tnews.mtr.com.hk/alert/ryg_line_status.xml",
                ),
                legend: env_or(
                    "MTR_LEGEND_URL",
                    "https://tnews.mtr.com.hk/alert/legend_remarks_v2.xml",
                ),
                alerts: env_or(
                    "MTR_ALERTS_URL",
                    "https://tnews.mtr.com.hk/alert/alert.json",
                ),
                late_cert: env_or(
                    "MTR_LATECERT_URL",
                    "https://tnews.mtr.com.hk/alert/latecert.json",
                ),
                line_config: env_or(
                    "MTR_LINE_CONFIG_URL",
                    "https://tnews.mtr.com.hk/alert/lineconfig.json",
                ),
                stations_api: env_or(
                    "MTR_STATIONS_API",
                    "https://proxy001.api.mtr.com.hk/ttp-api/v2/api/StationBaseInfo/HRStations/",
                ),
                journey_api: env_or(
                    "MTR_JOURNEY_API",
                    "https://proxy001.api.mtr.com.hk/ttp-api/v2/api/Path/HRRoutes/",
                ),
                ferry_routes: env_or(
                    "MTR_FERRY_ROUTES_URL",
                    "https://static.data.gov.hk/td/routes-fares-xml/ROUTE_FERRY.xml",
                ),
                hkkf_eta_base: env_or("MTR_HKKF_ETA_BASE", "https://www.hkkfeta.com/opendata/eta"),
                sun_ferry_eta: env_or("MTR_SUN_FERRY_ETA", "https://www.sunferry.com.hk/eta/"),
            },
            datasets: Datasets {
                lines_and_stations: "https://opendata.mtr.com.hk/data/mtr_lines_and_stations.csv"
                    .to_string(),
                lines_fares: "https://opendata.mtr.com.hk/data/mtr_lines_fares.csv".to_string(),
                light_rail_routes_and_stops:
                    "https://opendata.mtr.com.hk/data/light_rail_routes_and_stops.csv".to_string(),
                light_rail_fares: "https://opendata.mtr.com.hk/data/light_rail_fares.csv"
                    .to_string(),
                bus_routes: "https://opendata.mtr.com.hk/data/mtr_bus_routes.csv".to_string(),
                bus_stops: "https://opendata.mtr.com.hk/data/mtr_bus_stops.csv".to_string(),
                bus_fares: "https://opendata.mtr.com.hk/data/mtr_bus_fares.csv".to_string(),
                barrier_free: "https://opendata.mtr.com.hk/data/barrier_free_facilities.csv"
                    .to_string(),
                barrier_free_categories:
                    "https://opendata.mtr.com.hk/data/barrier_free_facility_category.csv"
                        .to_string(),
            },
            trip_planner_key: env_or("MTR_TTP_API_KEY", "d3dcfff7076444aaa8be8785bd352c5e"),
            timeout: Duration::from_millis(timeout_ms),
            cache: CacheConfig {
                static_data: Duration::from_secs(6 * 60 * 60),
                status: Duration::from_secs(20),
                alerts: Duration::from_secs(60),
                journey: Duration::from_secs(60),
            },
        }
    }
}
