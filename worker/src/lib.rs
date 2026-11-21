use std::sync::OnceLock;

use async_trait::async_trait;
use mtr_hk::app::App;
use mtr_hk::config::Config;
use mtr_hk::http::{HttpBackend, HttpError};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use worker::{
    Context, Env, Fetch, Headers, Method, Request, RequestInit, Response, Result, wasm_bindgen::JsValue,
};

const SERVER_NAME: &str = "mtr-hk";
const SERVER_VERSION: &str = "1.0.0";
const DEFAULT_PROTOCOL_VERSION: &str = "2025-06-18";

#[derive(Clone, Default)]
struct WorkerHttpClient;

#[async_trait]
impl HttpBackend for WorkerHttpClient {
    async fn get_text(&self, url: &str, headers: &[(&str, &str)]) -> Result<String, HttpError> {
        self.request(Method::Get, url, headers, None).await
    }

    async fn post_text(
        &self,
        url: &str,
        body: &Value,
        headers: &[(&str, &str)],
    ) -> Result<String, HttpError> {
        let serialized = body.to_string();
        self.request(Method::Post, url, headers, Some(&serialized))
            .await
    }
}

impl WorkerHttpClient {
    async fn request(
        &self,
        method: Method,
        url: &str,
        headers: &[(&str, &str)],
        body: Option<&str>,
    ) -> Result<String, HttpError> {
        let request_headers = Headers::new();
        request_headers
            .set("User-Agent", "mtr-hk-worker/1.0")
            .map_err(|error| request_error(url, error))?;
        request_headers
            .set("Accept", "application/json, text/xml, text/csv, */*")
            .map_err(|error| request_error(url, error))?;
        for (name, value) in headers {
            request_headers
                .set(name, value)
                .map_err(|error| request_error(url, error))?;
        }

        let mut init = RequestInit::new();
        init.with_method(method).with_headers(request_headers);
        if let Some(body) = body {
            init.with_body(Some(JsValue::from_str(body)));
        }

        let request =
            Request::new_with_init(url, &init).map_err(|error| request_error(url, error))?;
        let mut response = Fetch::Request(request)
            .send()
            .await
            .map_err(|error| request_error(url, error))?;
        let status = response.status_code();
        let text = response
            .text()
            .await
            .map_err(|error| request_error(url, error))?;

        if !(200..300).contains(&status) {
            let snippet = text
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .chars()
                .take(300)
                .collect::<String>();
            return Err(HttpError::Status {
                status,
                body: if snippet.is_empty() {
                    String::new()
                } else {
                    format!(": {snippet}")
                },
                url: url.to_string(),
            });
        }
        Ok(text)
    }
}

fn request_error(url: &str, error: impl std::fmt::Display) -> HttpError {
    HttpError::Request {
        url: url.to_string(),
        detail: error.to_string(),
    }
}

type WorkerApp = App<WorkerHttpClient>;

fn app() -> &'static WorkerApp {
    static APP: OnceLock<WorkerApp> = OnceLock::new();
    APP.get_or_init(|| App::with_http(Config::from_env(), WorkerHttpClient))
}

#[derive(Debug, Deserialize, JsonSchema)]
struct StatusArgs {
    #[serde(default)]
    line: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct AlertsArgs {
    #[serde(default)]
    line: Option<String>,
    #[serde(default)]
    lang: Option<String>,
    #[serde(default)]
    include_content: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct NextTrainsArgs {
    station: String,
    #[serde(default)]
    line: Option<String>,
    #[serde(default)]
    direction: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct JourneyArgs {
    from: String,
    to: String,
    #[serde(default)]
    max_routes: Option<u8>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct FaresArgs {
    from: String,
    to: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct StationsArgs {
    query: String,
    #[serde(default)]
    limit: Option<u8>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct LinesArgs {
    #[serde(default)]
    include_stations: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct FacilitiesArgs {
    station: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct LrNextArgs {
    stop: String,
    #[serde(default)]
    route: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct LrRoutesArgs {
    #[serde(default)]
    query: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct LrFaresArgs {
    from: String,
    to: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct BusNextArgs {
    route: String,
    #[serde(default)]
    stop: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct FerriesArgs {
    #[serde(default)]
    query: Option<String>,
    #[serde(default)]
    operator: Option<String>,
    #[serde(default)]
    include_routes: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct FerryEtaArgs {
    operator: String,
    route: String,
    #[serde(default)]
    direction: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct BusRoutesArgs {
    #[serde(default)]
    query: Option<String>,
    #[serde(default)]
    stop: Option<String>,
}

fn schema<T: JsonSchema>() -> Value {
    serde_json::to_value(schemars::schema_for!(T)).unwrap_or_else(|_| json!({ "type": "object" }))
}

fn tool(name: &str, description: &str, input_schema: Value) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": input_schema,
        "annotations": {
            "readOnlyHint": true,
            "destructiveHint": false,
            "idempotentHint": false,
            "openWorldHint": true
        }
    })
}

fn tools() -> Vec<Value> {
    vec![
        tool(
            "mtr_status",
            "Get live MTR line service status plus matching traffic notices.",
            schema::<StatusArgs>(),
        ),
        tool(
            "mtr_alerts",
            "Get current MTR traffic news, delay notices and LateCert records.",
            schema::<AlertsArgs>(),
        ),
        tool(
            "mtr_next_trains",
            "Get next trains at a heavy-rail station.",
            schema::<NextTrainsArgs>(),
        ),
        tool(
            "mtr_plan_journey",
            "Plan a heavy-rail journey between two stations.",
            schema::<JourneyArgs>(),
        ),
        tool(
            "mtr_fares",
            "Look up heavy-rail fares between two stations.",
            schema::<FaresArgs>(),
        ),
        tool(
            "mtr_stations",
            "Search MTR stations by English/Chinese name or code.",
            schema::<StationsArgs>(),
        ),
        tool(
            "mtr_lines",
            "List MTR lines and optionally their stations.",
            schema::<LinesArgs>(),
        ),
        tool(
            "mtr_station_facilities",
            "Get barrier-free facilities at an MTR station.",
            schema::<FacilitiesArgs>(),
        ),
        tool(
            "mtr_light_rail_next_trains",
            "Get live Light Rail predictions at a stop.",
            schema::<LrNextArgs>(),
        ),
        tool(
            "mtr_light_rail_routes",
            "List Light Rail routes or find routes serving a stop.",
            schema::<LrRoutesArgs>(),
        ),
        tool(
            "mtr_light_rail_fares",
            "Look up Light Rail fares between two stops.",
            schema::<LrFaresArgs>(),
        ),
        tool(
            "mtr_bus_next_trains",
            "Get live MTR Bus arrival estimates for a route.",
            schema::<BusNextArgs>(),
        ),
        tool(
            "mtr_bus_routes",
            "List or search MTR Bus routes and stops.",
            schema::<BusRoutesArgs>(),
        ),
        tool(
            "mtr_ferries",
            "List and search Hong Kong ferry operators and routes.",
            schema::<FerriesArgs>(),
        ),
        tool(
            "mtr_ferry_eta",
            "Get live HKKF or Sun Ferry next-sailing predictions.",
            schema::<FerryEtaArgs>(),
        ),
    ]
}

async fn call_tool(name: &str, arguments: Value) -> Result<Value, String> {
    let app = app();
    let result = match name {
        "mtr_status" => {
            let args: StatusArgs = serde_json::from_value(arguments).map_err(|e| e.to_string())?;
            mtr_hk::status::get_service_status(app, args.line.as_deref())
                .await
                .map_err(|e| e.to_string())
                .and_then(|value| serde_json::to_value(value).map_err(|e| e.to_string()))
        }
        "mtr_alerts" => {
            let args: AlertsArgs = serde_json::from_value(arguments).map_err(|e| e.to_string())?;
            let lang = if args.lang.as_deref() == Some("tc") {
                "tc"
            } else {
                "en"
            };
            mtr_hk::alerts::get_alerts(
                app,
                mtr_hk::alerts::AlertOptions {
                    line: args.line.as_deref(),
                    include_content: args.include_content.unwrap_or(true),
                    lang,
                    active_only: false,
                },
            )
            .await
            .map_err(|e| e.to_string())
            .and_then(|value| serde_json::to_value(value).map_err(|e| e.to_string()))
        }
        "mtr_next_trains" => {
            let args: NextTrainsArgs =
                serde_json::from_value(arguments).map_err(|e| e.to_string())?;
            mtr_hk::next_train::get_next_trains(
                app,
                &args.station,
                args.line.as_deref(),
                args.direction.as_deref(),
            )
            .await
            .map_err(|e| e.to_string())
            .and_then(|value| serde_json::to_value(value).map_err(|e| e.to_string()))
        }
        "mtr_plan_journey" => {
            let args: JourneyArgs = serde_json::from_value(arguments).map_err(|e| e.to_string())?;
            mtr_hk::journey::plan_journey(app, &args.from, &args.to, args.max_routes)
                .await
                .map_err(|e| e.to_string())
                .and_then(|value| serde_json::to_value(value).map_err(|e| e.to_string()))
        }
        "mtr_fares" => {
            let args: FaresArgs = serde_json::from_value(arguments).map_err(|e| e.to_string())?;
            mtr_hk::services::get_rail_fares(app, &args.from, &args.to)
                .await
                .map_err(|e| e.to_string())
        }
        "mtr_stations" => {
            let args: StationsArgs = serde_json::from_value(arguments).map_err(|e| e.to_string())?;
            mtr_hk::services::search_stations(app, &args.query, args.limit.unwrap_or(8))
                .await
                .map_err(|e| e.to_string())
        }
        "mtr_lines" => {
            let args: LinesArgs = serde_json::from_value(arguments).map_err(|e| e.to_string())?;
            mtr_hk::services::list_lines(app, args.include_stations.unwrap_or(false))
                .await
                .map_err(|e| e.to_string())
        }
        "mtr_station_facilities" => {
            let args: FacilitiesArgs =
                serde_json::from_value(arguments).map_err(|e| e.to_string())?;
            mtr_hk::services::get_station_facilities(app, &args.station)
                .await
                .map_err(|e| e.to_string())
        }
        "mtr_light_rail_next_trains" => {
            let args: LrNextArgs = serde_json::from_value(arguments).map_err(|e| e.to_string())?;
            mtr_hk::services::get_light_rail_next_trains(app, &args.stop, args.route.as_deref())
                .await
                .map_err(|e| e.to_string())
        }
        "mtr_light_rail_routes" => {
            let args: LrRoutesArgs = serde_json::from_value(arguments).map_err(|e| e.to_string())?;
            mtr_hk::services::get_light_rail_routes(app, args.query.as_deref())
                .await
                .map_err(|e| e.to_string())
        }
        "mtr_light_rail_fares" => {
            let args: LrFaresArgs = serde_json::from_value(arguments).map_err(|e| e.to_string())?;
            mtr_hk::services::get_light_rail_fares(app, &args.from, &args.to)
                .await
                .map_err(|e| e.to_string())
        }
        "mtr_bus_next_trains" => {
            let args: BusNextArgs = serde_json::from_value(arguments).map_err(|e| e.to_string())?;
            mtr_hk::services::get_bus_next_trains(app, &args.route, args.stop.as_deref())
                .await
                .map_err(|e| e.to_string())
        }
        "mtr_bus_routes" => {
            let args: BusRoutesArgs = serde_json::from_value(arguments).map_err(|e| e.to_string())?;
            mtr_hk::services::get_bus_routes(app, args.query.as_deref(), args.stop.as_deref())
                .await
                .map_err(|e| e.to_string())
        }
        "mtr_ferries" => {
            let args: FerriesArgs = serde_json::from_value(arguments).map_err(|e| e.to_string())?;
            mtr_hk::ferries::get_ferries(
                app,
                args.query.as_deref(),
                args.operator.as_deref(),
                args.include_routes.unwrap_or(false),
            )
            .await
            .map_err(|e| e.to_string())
        }
        "mtr_ferry_eta" => {
            let args: FerryEtaArgs = serde_json::from_value(arguments).map_err(|e| e.to_string())?;
            mtr_hk::ferries::get_ferry_eta(
                app,
                &args.operator,
                &args.route,
                args.direction.as_deref(),
            )
            .await
            .map_err(|e| e.to_string())
        }
        _ => return Err(format!("Unknown tool {name}")),
    };
    result
}

fn cors(mut response: Response) -> Result<Response> {
    let headers = response.headers_mut();
    headers.set("Access-Control-Allow-Origin", "*")?;
    headers.set("Access-Control-Allow-Headers", "Content-Type, Authorization, X-API-Key, Mcp-Session-Id")?;
    headers.set("Access-Control-Allow-Methods", "GET, POST, OPTIONS")?;
    headers.set("Access-Control-Expose-Headers", "Mcp-Session-Id")?;
    Ok(response)
}

fn json_response(value: &Value) -> Result<Response> {
    cors(Response::from_json(value)?)
}

fn rpc_result(id: Value, result: Value) -> Response {
    json_response(&json!({ "jsonrpc": "2.0", "id": id, "result": result }))
        .unwrap_or_else(|_| Response::error("serialization failed", 500).unwrap())
}

fn rpc_error(id: Value, code: i64, message: &str) -> Response {
    json_response(&json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message }
    }))
    .unwrap_or_else(|_| Response::error("serialization failed", 500).unwrap())
}

fn tool_result(value: Value) -> Value {
    let text = serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{}".to_string());
    json!({
        "content": [{ "type": "text", "text": text }],
        "structuredContent": value
    })
}

fn tool_error(message: String) -> Value {
    json!({
        "content": [{ "type": "text", "text": format!("Error: {message}") }],
        "isError": true
    })
}

fn authorized(req: &Request, env: &Env) -> bool {
    let Ok(expected) = env.var("MTR_MCP_API_KEY") else {
        return true;
    };
    let expected = expected.to_string();
    if expected.trim().is_empty() {
        return true;
    }
    let bearer = req
        .headers()
        .get("Authorization")
        .ok()
        .flatten()
        .and_then(|value| value.strip_prefix("Bearer ").map(str::to_string));
    let api_key = req.headers().get("X-API-Key").ok().flatten();
    bearer.is_some_and(|value| constant_time_eq(value.as_bytes(), expected.as_bytes()))
        || api_key.is_some_and(|value| constant_time_eq(value.as_bytes(), expected.as_bytes()))
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

async fn handle_rpc(req: Request) -> Result<Response> {
    let mut req = req;
    let payload: Value = match req.json().await {
        Ok(payload) => payload,
        Err(error) => return rpc_error(Value::Null, -32700, &format!("Parse error: {error}")),
    };

    // JSON-RPC batch support.
    if let Some(batch) = payload.as_array() {
        let mut responses = Vec::with_capacity(batch.len());
        for item in batch {
            responses.push(handle_one(item.clone()).await);
        }
        return json_response(&Value::Array(responses));
    }

    let response = handle_one(payload).await;
    Ok(rpc_result(Value::Null, response))
}

async fn handle_one(payload: Value) -> Value {
    let id = payload.get("id").cloned().unwrap_or(Value::Null);
    let method = payload.get("method").and_then(Value::as_str).unwrap_or("");
    let params = payload.get("params").cloned().unwrap_or_else(|| json!({}));

    if method.starts_with("notifications/") {
        return json!({});
    }

    match method {
        "initialize" => {
            let protocol = params
                .get("protocolVersion")
                .and_then(Value::as_str)
                .unwrap_or(DEFAULT_PROTOCOL_VERSION);
            json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "protocolVersion": protocol,
                    "capabilities": { "tools": {} },
                    "serverInfo": { "name": SERVER_NAME, "version": SERVER_VERSION },
                    "instructions": "Tools for Hong Kong MTR and ferries. All tools return JSON. Use mtr_stations to disambiguate station names. Live data comes from MTR and Transport Department public endpoints. If a tool returns an error, report it rather than inventing an arrival time or status."
                }
            })
        }
        "ping" => json!({ "jsonrpc": "2.0", "id": id, "result": {} }),
        "tools/list" => json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": { "tools": tools() }
        }),
        "tools/call" => {
            let name = params.get("name").and_then(Value::as_str).unwrap_or("");
            let arguments = params
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));
            let result = call_tool(name, arguments).await;
            match result {
                Ok(value) => json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": tool_result(value)
                }),
                Err(error) => json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": tool_error(error)
                }),
            }
        }
        _ => json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": -32601, "message": format!("Method not found: {method}") }
        }),
    }
}

#[worker::event(fetch)]
async fn fetch(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    let path = req.path();

    if req.method() == Method::Options {
        return cors(Response::empty()?.with_status(204));
    }

    if path == "/health" {
        return json_response(&json!({
            "status": "ok",
            "service": "mtr-hk",
            "transport": "streamable-http",
            "runtime": "cloudflare-workers"
        }));
    }

    if path != "/mcp" && !path.starts_with("/mcp/") {
        return cors(Response::error("Not found", 404)?);
    }

    if req.method() != Method::Post {
        return cors(Response::error("Use POST for Streamable HTTP MCP", 405)?);
    }

    if !authorized(&req, &env) {
        return cors(
            Response::error("unauthorized", 401)?
                .with_headers(Headers::new()),
        );
    }

    // handle_rpc consumes the body; notifications still return an empty 202.
    handle_rpc(req).await
}

#[cfg(test)]
mod tests {
    use super::constant_time_eq;

    #[test]
    fn api_keys_are_compared_without_early_exit() {
        assert!(constant_time_eq(b"secret", b"secret"));
        assert!(!constant_time_eq(b"secret", b"secreT"));
        assert!(!constant_time_eq(b"secret", b"secret-longer"));
    }
}
