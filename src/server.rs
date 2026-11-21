use std::future::Future;
use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use axum::Router;
use axum::extract::{Request, State};
use axum::http::{StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, ErrorData};
use rmcp::transport::stdio;
use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
};
use rmcp::{ServerHandler, ServiceExt, tool, tool_handler, tool_router};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use tower_http::cors::CorsLayer;

use crate::app::App;
use crate::http::HttpClient;

#[derive(Debug, Deserialize, JsonSchema)]
struct StatusArgs {
    /// Line code or name, e.g. TWL, Island Line, Tuen Ma Line, Light Rail.
    #[serde(default)]
    line: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct AlertsArgs {
    /// Filter to a line code such as TWL, EAL, TML, AEL or LR.
    #[serde(default)]
    line: Option<String>,
    /// Language for titles and alert content: en or tc.
    #[serde(default)]
    lang: Option<String>,
    /// Include the full notice body (default true).
    #[serde(default)]
    include_content: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct NextTrainsArgs {
    /// Station name in English or Chinese, or station code, e.g. Admiralty, 金鐘, ADM.
    station: String,
    /// Line code or name, e.g. TCL or Tung Chung Line.
    #[serde(default)]
    line: Option<String>,
    /// Optional direction filter: up or down.
    #[serde(default)]
    direction: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct JourneyArgs {
    /// Origin station name or code.
    from: String,
    /// Destination station name or code.
    to: String,
    /// Maximum number of route options to return (default 3).
    #[serde(default)]
    max_routes: Option<u8>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct FaresArgs {
    /// Origin station name or code.
    from: String,
    /// Destination station name or code.
    to: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct StationsArgs {
    /// Partial or full station name, or code, e.g. 'tsuen', 金鐘, ADM.
    query: String,
    /// Maximum number of matches (1-20, default 8).
    #[serde(default)]
    limit: Option<u8>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct LinesArgs {
    /// Include ordered station lists (default false).
    #[serde(default)]
    include_stations: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct FacilitiesArgs {
    /// Station name in English or Chinese, or station code.
    station: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct LrNextArgs {
    /// Light Rail stop name, code or numeric stop ID.
    stop: String,
    /// Optional Light Rail route number, e.g. 507.
    #[serde(default)]
    route: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct LrRoutesArgs {
    /// Light Rail route number, stop name, or stop code.
    #[serde(default)]
    query: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct LrFaresArgs {
    /// Origin Light Rail stop name or code.
    from: String,
    /// Destination Light Rail stop name or code.
    to: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct BusNextArgs {
    /// MTR Bus route number, e.g. 506, K12, K51.
    route: String,
    /// Optional stop name, partial name or stop ID.
    #[serde(default)]
    stop: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct FerriesArgs {
    /// Route, pier, island, district or operator name to search for.
    #[serde(default)]
    query: Option<String>,
    /// Operator id such as star-ferry, sun-ferry, hkkf, fortune-ferry or turbojet.
    #[serde(default)]
    operator: Option<String>,
    /// Include every matching route when no query is supplied (default false).
    #[serde(default)]
    include_routes: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct FerryEtaArgs {
    /// Operator id: hkkf or sun-ferry (other operators return timetable guidance).
    operator: String,
    /// Operator route code or MTR ferry route name/id, e.g. CECC, 1, Central - Cheung Chau.
    route: String,
    /// HKKF direction: outbound or inbound (default outbound).
    #[serde(default)]
    direction: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct BusRoutesArgs {
    /// Route number, route name, or partial match.
    #[serde(default)]
    query: Option<String>,
    /// Optional stop name filter.
    #[serde(default)]
    stop: Option<String>,
}

fn ok(value: Value) -> std::result::Result<CallToolResult, ErrorData> {
    let text = serde_json::to_string_pretty(&value)
        .map_err(|error| ErrorData::internal_error(error.to_string(), None))?;
    let mut result = CallToolResult::success(vec![ContentBlock::text(text)]);
    result.structured_content = Some(value);
    Ok(result)
}

fn fail(error: anyhow::Error) -> CallToolResult {
    CallToolResult::error(vec![ContentBlock::text(format!("Error: {error}"))])
}

async fn finish<F>(future: F) -> std::result::Result<CallToolResult, ErrorData>
where
    F: Future<Output = Result<Value>>,
{
    match future.await {
        Ok(value) => ok(value),
        Err(error) => Ok(fail(error)),
    }
}

#[derive(Clone)]
pub struct MtrServer {
    app: App<HttpClient>,
}

#[tool_router]
impl MtrServer {
    pub fn new(app: App<HttpClient>) -> Self {
        Self { app }
    }

    #[tool(
        name = "mtr_status",
        description = "Get live MTR line service status (normal, delay, disruption, special, typhoon, non-service hours) plus matching traffic notices. Use this first for questions like 'is the MTR running normally?' or 'are there delays on the Island Line?'. Returns all lines unless a line is specified. JSON includes last update time in GMT and HKT, status labels, colours, details URLs, a summary of delayed/disrupted line codes, notice count and matching alert summaries with severity.",
        annotations(
            title = "MTR service status",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = true
        )
    )]
    async fn mtr_status(
        &self,
        Parameters(args): Parameters<StatusArgs>,
    ) -> std::result::Result<CallToolResult, ErrorData> {
        finish(async {
            let value = crate::status::get_service_status(&self.app, args.line.as_deref()).await?;
            Ok(serde_json::to_value(value)?)
        })
        .await
    }

    #[tool(
        name = "mtr_alerts",
        description = "Get current MTR Traffic News: service disruption notifications, delay notices, planned engineering, early last trains and special arrangements. Each item is classified as disruption, delay, notice or record, includes affected line names and severity, and is converted from MTR's HTML into readable text. LateCert PDFs are returned as records, not live disruptions. Optionally filter by line code and set include_content=false for titles only.",
        annotations(
            title = "MTR traffic news and disruption notices",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = true
        )
    )]
    async fn mtr_alerts(
        &self,
        Parameters(args): Parameters<AlertsArgs>,
    ) -> std::result::Result<CallToolResult, ErrorData> {
        finish(async {
            let lang = if args.lang.as_deref() == Some("tc") {
                "tc"
            } else {
                "en"
            };
            let value = crate::alerts::get_alerts(
                &self.app,
                crate::alerts::AlertOptions {
                    line: args.line.as_deref(),
                    include_content: args.include_content.unwrap_or(true),
                    lang,
                    active_only: false,
                },
            )
            .await?;
            Ok(serde_json::to_value(value)?)
        })
        .await
    }

    #[tool(
        name = "mtr_next_trains",
        description = "Get the next up to four trains in each direction at a heavy-rail MTR station, with destination, platform, minutes away and delay flag. If the station is an interchange, all real-time lines are returned. Optionally restrict to one line and/or direction.",
        annotations(
            title = "MTR next trains",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = true
        )
    )]
    async fn mtr_next_trains(
        &self,
        Parameters(args): Parameters<NextTrainsArgs>,
    ) -> std::result::Result<CallToolResult, ErrorData> {
        finish(async {
            let value = crate::next_train::get_next_trains(
                &self.app,
                &args.station,
                args.line.as_deref(),
                args.direction.as_deref(),
            )
            .await?;
            Ok(serde_json::to_value(value)?)
        })
        .await
    }

    #[tool(
        name = "mtr_plan_journey",
        description = "Plan a heavy-rail journey between two MTR stations. Returns estimated journey time, interchange count, fares, first/last train times and step-by-step legs with boarding platform and direction. Uses MTR's own trip-planner API. Journey times are weekday-morning estimates.",
        annotations(
            title = "Plan an MTR journey",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = true
        )
    )]
    async fn mtr_plan_journey(
        &self,
        Parameters(args): Parameters<JourneyArgs>,
    ) -> std::result::Result<CallToolResult, ErrorData> {
        finish(async {
            let value =
                crate::journey::plan_journey(&self.app, &args.from, &args.to, args.max_routes)
                    .await?;
            Ok(serde_json::to_value(value)?)
        })
        .await
    }

    #[tool(
        name = "mtr_fares",
        description = "Look up adult, child, elderly, student, JoyYou and Persons with Disabilities fares between two heavy-rail stations, for Octopus and single-journey tickets. Airport Express fares fall back to MTR's trip planner. Prices are in HKD.",
        annotations(
            title = "MTR heavy-rail fares",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = true
        )
    )]
    async fn mtr_fares(
        &self,
        Parameters(args): Parameters<FaresArgs>,
    ) -> std::result::Result<CallToolResult, ErrorData> {
        finish(crate::services::get_rail_fares(
            &self.app, &args.from, &args.to,
        ))
        .await
    }

    #[tool(
        name = "mtr_stations",
        description = "Search heavy-rail MTR stations by English name, Traditional/Simplified Chinese name or station code. Returns station IDs, codes, both names, serving lines and coordinates. Use this to disambiguate a station before calling other tools.",
        annotations(
            title = "Search MTR stations",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = true
        )
    )]
    async fn mtr_stations(
        &self,
        Parameters(args): Parameters<StationsArgs>,
    ) -> std::result::Result<CallToolResult, ErrorData> {
        finish(crate::services::search_stations(
            &self.app,
            &args.query,
            args.limit.unwrap_or(8),
        ))
        .await
    }

    #[tool(
        name = "mtr_lines",
        description = "List MTR lines with code, bilingual name, official colour, station count and whether real-time data is available. Includes High Speed Rail (no real-time next-train feed) plus a Light Rail summary with route, direction and stop counts. Set include_stations=true for ordered heavy-rail stations and all Light Rail route stops.",
        annotations(
            title = "List MTR lines",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = true
        )
    )]
    async fn mtr_lines(
        &self,
        Parameters(args): Parameters<LinesArgs>,
    ) -> std::result::Result<CallToolResult, ErrorData> {
        finish(crate::services::list_lines(
            &self.app,
            args.include_stations.unwrap_or(false),
        ))
        .await
    }

    #[tool(
        name = "mtr_station_facilities",
        description = "Get barrier-free facilities at an MTR station from MTR's open data: lifts, ramps, wide gates, accessible toilets, tactile guidance, induction loops and more, including which exits have step-free access. This is static facility data, not live lift outage status.",
        annotations(
            title = "MTR station accessibility facilities",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = true
        )
    )]
    async fn mtr_station_facilities(
        &self,
        Parameters(args): Parameters<FacilitiesArgs>,
    ) -> std::result::Result<CallToolResult, ErrorData> {
        finish(crate::services::get_station_facilities(
            &self.app,
            &args.station,
        ))
        .await
    }

    #[tool(
        name = "mtr_light_rail_next_trains",
        description = "Get live MTR Light Rail predictions at a stop, grouped by platform. Shows route number, destination, minutes away and whether it is a special route. Use mtr_light_rail_routes to find a stop code or route.",
        annotations(
            title = "Light Rail next trains",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = true
        )
    )]
    async fn mtr_light_rail_next_trains(
        &self,
        Parameters(args): Parameters<LrNextArgs>,
    ) -> std::result::Result<CallToolResult, ErrorData> {
        finish(crate::services::get_light_rail_next_trains(
            &self.app,
            &args.stop,
            args.route.as_deref(),
        ))
        .await
    }

    #[tool(
        name = "mtr_light_rail_routes",
        description = "Explore MTR Light Rail. With no query, lists all routes, directions and stop counts. With a route number (e.g. 507), returns its ordered stops. With a stop name, stop code or numeric stop ID, returns the routes serving that stop.",
        annotations(
            title = "Light Rail routes and stops",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = true
        )
    )]
    async fn mtr_light_rail_routes(
        &self,
        Parameters(args): Parameters<LrRoutesArgs>,
    ) -> std::result::Result<CallToolResult, ErrorData> {
        finish(crate::services::get_light_rail_routes(
            &self.app,
            args.query.as_deref(),
        ))
        .await
    }

    #[tool(
        name = "mtr_light_rail_fares",
        description = "Look up MTR Light Rail Octopus and single-journey fares between two Light Rail stops. Prices are in HKD.",
        annotations(
            title = "Light Rail fares",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = true
        )
    )]
    async fn mtr_light_rail_fares(
        &self,
        Parameters(args): Parameters<LrFaresArgs>,
    ) -> std::result::Result<CallToolResult, ErrorData> {
        finish(crate::services::get_light_rail_fares(
            &self.app, &args.from, &args.to,
        ))
        .await
    }

    #[tool(
        name = "mtr_bus_next_trains",
        description = "Get live MTR Bus / feeder bus arrival estimates for a route, optionally filtered to one stop. Returns per-stop predictions, direction, delay flag, suspension status and route fares. Use mtr_bus_routes to find routes and stops.",
        annotations(
            title = "MTR Bus next buses",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = true
        )
    )]
    async fn mtr_bus_next_trains(
        &self,
        Parameters(args): Parameters<BusNextArgs>,
    ) -> std::result::Result<CallToolResult, ErrorData> {
        finish(crate::services::get_bus_next_trains(
            &self.app,
            &args.route,
            args.stop.as_deref(),
        ))
        .await
    }

    #[tool(
        name = "mtr_ferries",
        description = "List and search Hong Kong ferry operators and routes. Covers the Star Ferry, Sun Ferry, Hong Kong & Kowloon Ferry, Discovery Bay, Park Island, Fortune Ferry, Chuen Kee, Coral Sea, Tsui Wah, Peng Chau Kai To, kaito/water-taxi services and cross-boundary Macao/Guangdong operators. Returns operator websites, scope, route counts, origin/destination, journey time, full fare and source links from the Transport Department dataset. Use mtr_ferry_eta for live HKKF and Sun Ferry predictions.",
        annotations(
            title = "Hong Kong ferry operators and routes",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = true
        )
    )]
    async fn mtr_ferries(
        &self,
        Parameters(args): Parameters<FerriesArgs>,
    ) -> std::result::Result<CallToolResult, ErrorData> {
        finish(crate::ferries::get_ferries(
            &self.app,
            args.query.as_deref(),
            args.operator.as_deref(),
            args.include_routes.unwrap_or(false),
        ))
        .await
    }

    #[tool(
        name = "mtr_ferry_eta",
        description = "Get live next-sailing predictions for Hong Kong ferries that publish a public ETA API: Hong Kong & Kowloon Ferry (HKKF) and Sun Ferry. Other operators return their timetable and official source link because they do not expose live ETA data. Use mtr_ferries first to find the operator route code.",
        annotations(
            title = "Live Hong Kong ferry ETA",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = true
        )
    )]
    async fn mtr_ferry_eta(
        &self,
        Parameters(args): Parameters<FerryEtaArgs>,
    ) -> std::result::Result<CallToolResult, ErrorData> {
        finish(crate::ferries::get_ferry_eta(
            &self.app,
            &args.operator,
            &args.route,
            args.direction.as_deref(),
        ))
        .await
    }

    #[tool(
        name = "mtr_bus_routes",
        description = "List or search MTR Bus and feeder bus routes. With no query, lists all routes with fares. With a route number, returns variants, fares and every stop in both directions. A stop name returns every route serving that stop, so this also answers 'which MTR bus goes to this stop?'. Use the optional stop filter to narrow stops within a route.",
        annotations(
            title = "MTR Bus routes and stops",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = true
        )
    )]
    async fn mtr_bus_routes(
        &self,
        Parameters(args): Parameters<BusRoutesArgs>,
    ) -> std::result::Result<CallToolResult, ErrorData> {
        finish(crate::services::get_bus_routes(
            &self.app,
            args.query.as_deref(),
            args.stop.as_deref(),
        ))
        .await
    }
}

#[tool_handler(
    name = "mtr-hk",
    version = "1.0.0",
    instructions = "Tools for Hong Kong MTR: live service status and delays, traffic notices, next trains, journey planning, fares, Light Rail, MTR Bus and station accessibility. All tools return JSON. Times are Hong Kong local time. Use mtr_stations first when a station name is ambiguous. Live data comes from MTR's public open-data APIs and the endpoints used by MTR's own website/app. Fares and journey times are estimates, not guarantees. If a tool returns an error, report it rather than inventing an arrival time or service status."
)]
impl ServerHandler for MtrServer {}

pub async fn run_stdio() -> Result<()> {
    let app = App::new()?;
    let service = MtrServer::new(app)
        .serve(stdio())
        .await
        .map_err(|error| anyhow::anyhow!("{error}"))?;
    service
        .waiting()
        .await
        .map_err(|error| anyhow::anyhow!("{error}"))?;
    Ok(())
}

async fn health() -> axum::Json<Value> {
    axum::Json(json!({
        "status": "ok",
        "service": "mtr-hk-mcp",
        "transport": "streamable-http",
        "endpoint": "/mcp"
    }))
}

#[derive(Clone)]
struct AuthConfig {
    key: Option<String>,
}

async fn require_api_key(
    State(config): State<AuthConfig>,
    request: Request,
    next: Next,
) -> Response {
    let Some(expected) = config.key.as_deref() else {
        return next.run(request).await;
    };
    if request.method() == axum::http::Method::OPTIONS {
        return next.run(request).await;
    }
    let authorized = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .is_some_and(|value| constant_time_eq(value.as_bytes(), expected.as_bytes()))
        || request
            .headers()
            .get("x-api-key")
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| constant_time_eq(value.as_bytes(), expected.as_bytes()));
    if authorized {
        next.run(request).await
    } else {
        (
            StatusCode::UNAUTHORIZED,
            [(header::WWW_AUTHENTICATE, "Bearer")],
            "unauthorized",
        )
            .into_response()
    }
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut difference = 0u8;
    for (left, right) in left.iter().zip(right) {
        difference |= left ^ right;
    }
    difference == 0
}

fn configured_api_key() -> Option<String> {
    ["MTR_MCP_API_KEY", "MTR_API_KEY", "MTR_AUTH_TOKEN"]
        .into_iter()
        .find_map(|name| std::env::var(name).ok())
        .filter(|value| !value.trim().is_empty())
}

/// Serve MCP over the Streamable HTTP transport. The same endpoint accepts
/// JSON responses and server-sent event streams, so remote clients that use
/// either HTTP or SSE framing can connect.
pub async fn run_http(addr: SocketAddr) -> Result<()> {
    let app = App::new()?;
    let service = StreamableHttpService::new(
        move || Ok(MtrServer::new(app.clone())),
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig::default(),
    );

    let auth = AuthConfig {
        key: configured_api_key(),
    };
    if auth.key.is_some() {
        eprintln!("mtr-hk MCP: API-key authentication enabled");
    }
    let protected = Router::new()
        .nest_service("/mcp", service)
        .layer(middleware::from_fn_with_state(auth, require_api_key));
    let router = Router::new()
        .route("/health", get(health))
        .merge(protected)
        .layer(CorsLayer::permissive());

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, router)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}

pub async fn run_http_from_env() -> Result<()> {
    let raw = std::env::var("MTR_HTTP_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_string());
    let addr = raw
        .parse::<SocketAddr>()
        .map_err(|error| anyhow::anyhow!("invalid MTR_HTTP_ADDR {raw:?}: {error}"))?;
    run_http(addr).await
}

#[cfg(test)]
mod tests {
    use super::constant_time_eq;

    #[test]
    fn compares_api_keys_without_early_exit() {
        assert!(constant_time_eq(b"secret", b"secret"));
        assert!(!constant_time_eq(b"secret", b"secreT"));
        assert!(!constant_time_eq(b"secret", b"secret-longer"));
        assert!(!constant_time_eq(b"", b"secret"));
    }
}
