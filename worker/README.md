# Pure-WASM Cloudflare Worker

This directory is the **primary pure `wasm32-unknown-unknown` Worker** for the
MTR and ferry MCP server. It runs the shared `mtr-hk` service layer with
Cloudflare `fetch` replacing `reqwest`.

Use this deployment for Cloudflare: no container image, no Docker engine and no
Tokio runtime. The older container-backed Worker remains available in
[`../cloudflare/`](../cloudflare/README.md) when you need the native HTTP server
and its legacy SSE transport.

## What is included

- Streamable HTTP MCP over `POST /mcp`
- `GET /health`
- The same 15 tools as the native server:
  `mtr_status`, `mtr_alerts`, `mtr_next_trains`,
  `mtr_plan_journey`, `mtr_fares`, `mtr_stations`, `mtr_lines`,
  `mtr_station_facilities`, `mtr_light_rail_next_trains`,
  `mtr_light_rail_routes`, `mtr_light_rail_fares`,
  `mtr_bus_next_trains`, `mtr_bus_routes`, `mtr_ferries`,
  `mtr_ferry_eta`
- Optional API-key auth through `MTR_MCP_API_KEY`
- CORS for browser-based MCP clients
- No durable cache; each isolate keeps the shared `App` TTL caches in memory

This Worker does not include the CLI or a stdio transport.

## Prerequisites

- Rust 1.89 with the WASM target:

  ```bash
  rustup toolchain install 1.89.0
  rustup target add wasm32-unknown-unknown --toolchain 1.89.0
  ```

- `worker-build` 0.8:

  ```bash
  cargo install worker-build@0.8.0
  ```

- Node.js, `npx` and a Cloudflare account for deployment.

The local `build.sh` pins Rust 1.89 when that toolchain is available, because a
Homebrew `rustc` on `PATH` can shadow rustup and lack the WASM target.

## Build and check

```bash
cd worker

# Fast type/build check without producing a bundle
cargo check --target wasm32-unknown-unknown --locked

# Produce the Worker bundle that wrangler deploys
bash build.sh
```

## Deploy

From this directory:

```bash
npx wrangler secret put MTR_MCP_API_KEY   # optional
npx wrangler deploy
```

`wrangler.jsonc` runs `bash build.sh` before packaging. The endpoint is:

```text
https://<worker-name>.<account>.workers.dev/mcp
```

Connect a client:

```bash
claude mcp add --transport http mtr-hk \
  https://<worker-name>.<account>.workers.dev/mcp
```

## Authentication

`MTR_MCP_API_KEY` is optional. When it is set to a non-empty value, `/mcp`
requires either:

```text
Authorization: Bearer <key>
X-API-Key: <key>
```

`/health` remains public. Comparison is length-checked and constant-time over
the compared bytes. With no key the endpoint is unauthenticated, so put
Cloudflare Access or OAuth in front of a public deployment when you need
per-user identity.

## Local Worker check

```bash
cd worker
npx wrangler dev --port 8787
```

Then use `http://localhost:8787/mcp` as the Worker MCP URL. The port is the
Worker's local listener; there is no separate container port. The same check
works from the repository root with its `wrangler.jsonc`.

## Architecture

| Path | Purpose |
| --- | --- |
| `src/lib.rs` | Worker entry point, MCP JSON-RPC handling, tools, CORS and auth |
| `build.sh` | Pins Rust 1.89 where available and runs `worker-build --release` |
| `wrangler.jsonc` | Standalone Worker name, build command and compatibility date |
| `Cargo.toml` | WASM crate that reuses the root `mtr-hk-mcp` library |

`WorkerHttpClient` implements the shared `HttpBackend` trait with
Cloudflare `fetch`. MTR parsing, caches and service functions live in `../src`
and are not duplicated here. The root [`wrangler.jsonc`](../wrangler.jsonc)
deploys this same Worker from the repository root; the legacy container config
is [`cloudflare/wrangler.container.jsonc`](../cloudflare/wrangler.container.jsonc).

## Limits

- This is a second tool implementation surface. When a tool is added to the
  native server, add it to `src/lib.rs` as well.
- There is no container-level `/health` probe, persistent SQLite store or stdio
  transport.
- Cloudflare Workers have CPU and request limits; large ferry and timetable
  responses can be slower than the native server on a cold isolate.
- The Worker uses the same unofficial public endpoints as the native server and
  is not an official MTR or Transport Department service.
- Legacy SSE is not exposed; use Streamable HTTP.
