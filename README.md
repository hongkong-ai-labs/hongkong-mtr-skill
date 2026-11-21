# Hong Kong MTR + ferry — MCP server + agent skill

A comprehensive Hong Kong transport integration for AI agents, written in Rust:

- **MCP server** (stdio and Streamable HTTP/SSE) with **15 tools** for live MTR status, delays, notices, next trains, journey planning, fares, Light Rail, MTR Bus, station accessibility, ferry operators/routes and live ferry ETAs.
- **Agent skill** (`skills/mtr/SKILL.md`) that teaches an agent when and how to use each tool, with a CLI fallback.
- **CLI** sharing the same service layer for scripting and manual checks.

MTR data comes from MTR's public open data plus the traffic-news and trip-planner endpoints used by MTR's own website/app. Ferry route/fare data comes from the Transport Department; live ferry ETAs come from HKKF and Sun Ferry. No browser automation, no login and no data API key are required; the HTTP MCP transport can optionally require a static API key.

## Quick start

Requires **Rust 1.89+** (`rustup` recommended).

```bash
cargo install --path .
mtr-hk status
mtr-hk ferries
```

This installs three binaries into `~/.cargo/bin`:

- `mtr-hk-mcp` — MCP stdio server
- `mtr-hk-mcp-http` — MCP Streamable HTTP/SSE server
- `mtr-hk` — CLI

If the commands are not found, add Cargo's bin directory to `PATH`:

```bash
export PATH="$HOME/.cargo/bin:$PATH"
```

For development without installing:

```bash
cargo build --release --bins
./target/release/mtr-hk status
./target/release/mtr-hk-mcp
```

### Other install routes

Prebuilt binaries for macOS (Intel or Apple silicon) and Linux (x86_64 or
aarch64, static musl) are checksum-verified by the installer:

```bash
curl -fsSL https://raw.githubusercontent.com/hongkong-ai-labs/hongkong-mtr-skill/main/scripts/install.sh | sh
```

As an agent skill:

```bash
npx skills add hongkong-ai-labs/hongkong-mtr-skill
```

Docker, Apple container and a pure Cloudflare Workers (V8 isolate) deployment
are described below. The native binaries keep stdio and Streamable HTTP/SSE;
the Worker reuses the same Rust service layer through a Workers `fetch` backend.

### Register the MCP server

The included `.mcp.json` uses the installed binary:

```json
{
  "mcpServers": {
    "mtr-hk": {
      "command": "mtr-hk-mcp",
      "args": []
    }
  }
}
```

For Claude Code, Claude Desktop, Cursor or any stdio MCP client, use the same command. If the client does not inherit your shell `PATH`, set `command` to the absolute path, e.g. `/Users/you/.cargo/bin/mtr-hk-mcp`.

### Remote HTTP/SSE MCP

The HTTP binary exposes the same 15 tools over Streamable HTTP at `/mcp`, with a `/health` endpoint:

```bash
MTR_HTTP_ADDR=0.0.0.0:8080 MTR_MCP_API_KEY="$SECRET" mtr-hk-mcp-http
```

```bash
claude mcp add --transport http mtr-hk http://localhost:8080/mcp
```

The `/mcp` endpoint returns JSON or SSE-framed responses depending on the client. Set `MTR_MCP_API_KEY` (or `MTR_API_KEY` / `MTR_AUTH_TOKEN`) to require `Authorization: Bearer <key>` or `X-API-Key: <key>`; `/health` stays public. Without a key there is no authentication, so do not expose it directly to the public internet.

### Containers

Build one image containing the CLI and both MCP transports:

```bash
docker build -t mtr-hk-mcp .
docker run --rm -p 8080:8080 mtr-hk-mcp
```

Run the stdio server instead:

```bash
docker run --rm -i --entrypoint mtr-hk-mcp mtr-hk-mcp
```

Or use Compose:

```bash
docker compose up --build http
```

Apple's `container` runtime uses the same OCI image:

```bash
container build -t mtr-hk-mcp .
container run --rm -p 8080:8080 mtr-hk-mcp
container run --rm -i --entrypoint mtr-hk-mcp mtr-hk-mcp
```

The image defaults to the HTTP server on port 8080. It runs as a non-root user and includes a `/health` check.

### Cloudflare Workers (pure WASM)

The root `wrangler.jsonc` deploys a true `wasm32-unknown-unknown` Worker on the
Cloudflare V8 isolate runtime. It does not use a Container, Docker, Tokio or
TCP; the shared service layer talks to MTR through the Workers `fetch` API and
exposes the same 15 MCP tools at `/mcp`. Native stdio and the full
Streamable HTTP/SSE server remain available from the binaries above.

Prerequisites: Rust 1.89 with the WASM target, `worker-build`, Node.js and a
Cloudflare account.

```bash
rustup target add wasm32-unknown-unknown --toolchain 1.89.0
cargo install worker-build@0.8.0
npx wrangler login
npx wrangler secret put MTR_MCP_API_KEY   # optional
npx wrangler deploy
```

The MCP endpoint is:

```text
https://<worker-name>.<account>.workers.dev/mcp
```

```bash
claude mcp add --transport http mtr-hk \
  https://<worker-name>.<account>.workers.dev/mcp
```

When `MTR_MCP_API_KEY` is set, `/mcp` requires `Authorization: Bearer <key>` or
`X-API-Key: <key>`; `/health` stays public. The Worker is stateless across
requests except for per-isolate caches, and it accepts POST requests at `/mcp`
(Streamable HTTP JSON responses). It intentionally has no stdio, CLI or legacy
SSE endpoint.

Run the Worker locally without Docker:

```bash
npx wrangler dev --port 8787
curl http://localhost:8787/health
```

Use `--var MTR_MCP_API_KEY:test-key` to exercise authentication locally. See
[`cloudflare/README.md`](cloudflare/README.md) for the Worker layout and
operational notes.

### Cloudflare Container (legacy fallback)

The earlier container-backed Worker is still available for accounts that need
the native HTTP server inside a Cloudflare Container. It requires Docker and
Containers enabled on the account:

```bash
cd cloudflare
npx wrangler secret put MTR_MCP_API_KEY -c wrangler.container.jsonc   # optional
npx wrangler deploy -c wrangler.container.jsonc
```

### Install the skill

```bash
mkdir -p ~/.claude/skills
ln -sfn "$PWD/skills/mtr" ~/.claude/skills/mtr
```

The repository also contains `.claude/skills/mtr` as a symlink, so project-local clients discover it.

## Tools

| Tool | What it returns |
|---|---|
| `mtr_status` | Live line status for all 10 metro lines + Light Rail, overall condition, delay/disruption summary, HKT last update, matching notices |
| `mtr_alerts` | Current traffic news with severity, affected line names, full content, disruption PDFs and planned works |
| `mtr_next_trains` | Next up to 4 trains per direction at a station, destination, platform, delay flag |
| `mtr_plan_journey` | Fastest routes with time, interchange count, fares and step-by-step legs |
| `mtr_fares` | Heavy-rail fare matrix by passenger category, Octopus vs single journey |
| `mtr_stations` | Search stations by English/Chinese name or code, with lines and coordinates |
| `mtr_lines` | Line codes, bilingual names, colours, station counts, real-time availability, plus a Light Rail summary |
| `mtr_station_facilities` | Barrier-free facilities: lifts, ramps, wide gates, accessible toilets, tactile guidance |
| `mtr_light_rail_next_trains` | Light Rail predictions by platform, route, destination, special-route flag and remarks |
| `mtr_light_rail_routes` | All Light Rail routes, ordered stops, partial route lookup, or routes serving a stop |
| `mtr_light_rail_fares` | Light Rail Octopus and single-journey fares |
| `mtr_bus_next_trains` | MTR Bus/feeder bus arrival estimates, route status, suspension flags and fares |
| `mtr_bus_routes` | Bus route or stop search, variants, fares and stops in both directions |
| `mtr_ferries` | 16 ferry operators and 58 TD routes; search by route, pier, island, district or operator |
| `mtr_ferry_eta` | Live next sailings for HKKF and Sun Ferry; timetable/source guidance for other operators |

All tools return JSON. Station, line and ferry arguments accept English, Traditional Chinese, Simplified Chinese and codes. Ambiguous input returns a tool error with suggestions instead of guessing.

## Light Rail and MTR Bus coverage

**Light Rail (LR)**
- Live LR line status and delay notices via `mtr_status` / `mtr_alerts` (`LR`).
- Next trains at any of the 68 stops, grouped by platform, including special routes.
- All 11 routes and 22 directions with ordered stops; partial route lookup (`51` → 751) and stop-based route discovery.
- Octopus and single-journey LR fares between any two stops.

**MTR Bus and feeder buses**
- All MTR Bus/feeder routes from the open-data route, stop and fare datasets.
- Next-bus estimates per stop, with direction, delay flag, scheduled flag, suspension status and route-level service remarks.
- Route discovery by route number/name **or stop name** (e.g. “Melody Garden” → route 506).
- Route variants, both directions, ordered stops, coordinates and fares.

Heavy rail, Light Rail and bus data are separate modes: station/stop names can repeat (e.g. Tuen Mun), so the tool and skill state which mode is being reported.

## Ferry coverage

`mtr_ferries` combines the Transport Department's franchised/licensed ferry route and fare dataset with a curated operator directory.

**16 operators covered:**

- Local scheduled: Star Ferry, Sun Ferry, Hong Kong & Kowloon Ferry (HKKF), Discovery Bay Transportation, Park Island Transport, Fortune Ferry, Chuen Kee, Coral Sea, Tsui Wah, Peng Chau Kai To.
- Kaito/water taxi: Kaito / licensed small ferries, Hong Kong Water Taxi.
- Cross-boundary: TurboJET, Cotai Water Jet, Chu Kong Passenger Transport.
- Dangerous-goods-only: Hongkong & Yaumati Ferry.

**Route/fare fields:** operator, route name in English/Traditional/Simplified Chinese, district, journey minutes, origin/destination, published adult full fare, TD source link and live-ETA availability. At the time of writing the dataset has 58 routes across Victoria Harbour, outlying islands, kaito and water-taxi services.

**Live ETAs:** only HKKF and Sun Ferry expose public ETA APIs integrated here.

```bash
mtr-hk ferry-eta sun-ferry CECC
mtr-hk ferry-eta hkkf 1 --direction=outbound
```

Star Ferry, Fortune Ferry, Park Island, Discovery Bay, kaito and cross-boundary operators do not provide a public live ETA API; the tool returns `live: false` with the official timetable/source link. Kaito/water-taxi services can be seasonal, bookable or demand-responsive.

## Real-time news and delays

- **Line status** comes from MTR's traffic-news status feed behind its service-status page. `overall` is `normal`, `delay`, `disruption`, `special`, `typhoon` or `non-service`, with a `summary` listing the affected line codes and a notice count. Timestamps are returned in GMT and HKT, plus the upstream refresh interval.
- **Traffic news** comes from MTR's `alert.json` and `latecert.json`. Each item is classified as `disruption`, `delay`, `notice` or `record`, includes affected line codes and names, and carries the full notice body. `hasMap`, `hasShuttleBus` and `hasAlternativeTransport` flag notices with extra arrangements; LateCert items link to MTR's disruption-notification PDF.
- **Next-train delay flags** are surfaced by `mtr_next_trains` (`delayed`), and Light Rail status/notices use the same LR status feed.
- **Bus service status** is surfaced by `mtr_bus_next_trains`: route status code/colour/remarks, refresh time, per-stop suspension flags and `suspendedStopCount`.
- Historical LateCert records are kept by MTR after service normalises. They are marked `record` and are **not** treated as current disruptions; the live status feed is authoritative.

## CLI

```bash
mtr-hk status
mtr-hk alerts --lang=tc
mtr-hk next "Tsim Sha Tsui" --line=TWL
mtr-hk journey Admiralty "Tung Chung"
mtr-hk fares "Hong Kong" Airport
mtr-hk stations 金鐘
mtr-hk lines --stations
mtr-hk facilities Admiralty
mtr-hk lr-next "Tuen Mun Ferry Pier" --route=507
mtr-hk lr-routes 507
mtr-hk lr-routes "Tin King"
mtr-hk lr-fares "Tuen Mun Ferry Pier" "Tin King"
mtr-hk bus-next 506 --stop="Tuen Mun Ferry Pier"
mtr-hk bus-routes K12
mtr-hk bus-routes "Melody Garden"
mtr-hk ferries
mtr-hk ferries "Cheung Chau"
mtr-hk ferries --operator=sun-ferry
mtr-hk ferry-eta sun-ferry CECC
mtr-hk ferry-eta hkkf 1
mtr-hk update --check
```

Add `--json` for structured output. From the skill, use the wrapper:

```bash
skills/mtr/scripts/mtr status
```

Full CLI reference: [`skills/mtr/references/cli.md`](skills/mtr/references/cli.md).

## Data sources

| Data | Source |
|---|---|
| Heavy-rail next trains | `https://rt.data.gov.hk/v1/transport/mtr/getSchedule.php` (data.gov.hk) |
| Light Rail next trains | `https://rt.data.gov.hk/v1/transport/mtr/lrt/getSchedule` |
| MTR Bus ETA | `https://rt.data.gov.hk/v1/transport/mtr/bus/getSchedule` |
| Live line status | `https://tnews.mtr.com.hk/alert/ryg_line_status.xml` — the feed behind MTR's service-status page |
| Traffic news / disruption notices | `https://tnews.mtr.com.hk/alert/alert.json` + `latecert.json` |
| Journey planner | `https://proxy001.api.mtr.com.hk/ttp-api/v2/api/Path/HRRoutes/` — the API used by MTR's web trip planner |
| Station/line catalogue | `HRStations/` trip-planner API, with CSV fallback |
| MTR fares, routes, stops, accessibility | MTR open-data CSVs under `https://opendata.mtr.com.hk/data/` |
| Ferry routes and full fares | `https://static.data.gov.hk/td/routes-fares-xml/ROUTE_FERRY.xml` |
| HKKF live ETA | `https://www.hkkfeta.com/opendata/route/` and `/opendata/eta/{route_id}/{direction}` |
| Sun Ferry live ETA | `https://www.sunferry.com.hk/eta/?route={route_code}` |

Official datasets:

- [Real-time MTR train information](https://data.gov.hk/en-data/dataset/mtr-data2-nexttrain-data)
- [MTR routes, fares and barrier-free facilities](https://data.gov.hk/en-data/dataset/mtr-data-routes-fares-barrier-free-facilities)
- [Real-time MTR Bus and feeder bus schedule](https://data.gov.hk/en-data/dataset/mtr-mtr_bus-mtr-bus-eta-data)
- [Transport Department ferry routes and fares](https://data.gov.hk/en-data/dataset/hk-td-tis_14-routes-fares-xml)

Detailed endpoint notes, response shapes, ferry sources and caveats: [`skills/mtr/references/api-notes.md`](skills/mtr/references/api-notes.md).

## Configuration

Every endpoint can be overridden with environment variables:

| Variable | Default |
|---|---|
| `MTR_NEXT_TRAIN_URL` | `https://rt.data.gov.hk/v1/transport/mtr/getSchedule.php` |
| `MTR_LIGHT_RAIL_URL` | `https://rt.data.gov.hk/v1/transport/mtr/lrt/getSchedule` |
| `MTR_BUS_URL` | `https://rt.data.gov.hk/v1/transport/mtr/bus/getSchedule` |
| `MTR_STATUS_URL` | `https://tnews.mtr.com.hk/alert/ryg_line_status.xml` |
| `MTR_LEGEND_URL` | `https://tnews.mtr.com.hk/alert/legend_remarks_v2.xml` |
| `MTR_ALERTS_URL` | `https://tnews.mtr.com.hk/alert/alert.json` |
| `MTR_LATECERT_URL` | `https://tnews.mtr.com.hk/alert/latecert.json` |
| `MTR_LINE_CONFIG_URL` | `https://tnews.mtr.com.hk/alert/lineconfig.json` |
| `MTR_STATIONS_API` | `https://proxy001.api.mtr.com.hk/ttp-api/v2/api/StationBaseInfo/HRStations/` |
| `MTR_JOURNEY_API` | `https://proxy001.api.mtr.com.hk/ttp-api/v2/api/Path/HRRoutes/` |
| `MTR_FERRY_ROUTES_URL` | `https://static.data.gov.hk/td/routes-fares-xml/ROUTE_FERRY.xml` |
| `MTR_HKKF_ETA_BASE` | `https://www.hkkfeta.com/opendata/eta` |
| `MTR_SUN_FERRY_ETA` | `https://www.sunferry.com.hk/eta/` |
| `MTR_TTP_API_KEY` | Public key from MTR's front-end config (override if rotated) |
| `MTR_HTTP_TIMEOUT_MS` | `12000` |

The trip-planner key is a public client key embedded in MTR's own `config.json`; it is not a user secret. It can change, hence the override.

## Architecture

```
src/
  config.rs              endpoint + cache configuration
  constants.rs           stable MTR line metadata
  model.rs               normalized domain types
  app.rs                 shared app object and TTL caches
  http.rs                HTTP backend trait + native reqwest client
  cache.rs               typed TTL cache and de-duplication
  csvutil.rs             RFC-4180 CSV helpers
  text.rs                alert HTML -> readable text
  network.rs             station/line catalogue + CSV fallback
  fares.rs               heavy-rail fare matrix
  light_rail.rs          Light Rail routes/stops/fares
  bus.rs                 MTR Bus routes/stops/fares
  facilities.rs          barrier-free facilities
  alerts.rs              traffic news and LateCert records
  status.rs              live service status + notice merge
  next_train.rs          heavy-rail next trains
  journey.rs             trip planner + leg grouping
  services.rs            shared high-level MTR tool services
  ferries.rs             ferry operators, TD routes/fares, live HKKF/Sun ETA
  server.rs              MCP tool registration + stdio/HTTP transports
  cli.rs                 CLI
  bin/mtr-hk-mcp.rs      MCP stdio entry point
  bin/mtr-hk-mcp-http.rs MCP Streamable HTTP/SSE entry point
  bin/mtr-hk.rs          CLI entry point
src/lib.rs               library root (native modules feature-gated)
worker/                  pure Workers WASM crate + MCP JSON-RPC handler
worker/build.sh          WASM build pinned to Rust 1.89
wrangler.jsonc           pure Cloudflare Workers config
Dockerfile               OCI image for CLI + stdio + HTTP MCP
compose.yaml             local HTTP and stdio container profiles
cloudflare/src/          legacy Container front door and lifecycle
cloudflare/README.md     Worker deployment notes
cloudflare/wrangler.container.jsonc  legacy Container config
skills/mtr/
  SKILL.md               agent instructions
  references/            API and CLI documentation
  scripts/mtr            CLI wrapper
scripts/
  smoke.sh               live checks for every data source
  mcp_smoke.py           MCP stdio handshake + tool-call check
  mcp_http_smoke.py      MCP Streamable HTTP/SSE handshake + tool-call check
```

Caching: static datasets 6 hours, status 20 seconds, alerts 60 seconds, journey plans 60 seconds. Ferry routes/fares 6 hours. Next-train/LR/bus and ferry ETA predictions are not cached. Concurrent identical loads are de-duplicated.

## Releasing

CI runs formatting, clippy with warnings as errors, tests, a release build of all
three binaries, a `wasm32-unknown-unknown` check of `worker/`, and validates both
`SKILL.md` files plus the shell scripts.
Dependabot proposes weekly Cargo and GitHub Actions updates, and the audit
workflow checks the lockfile against the RustSec advisory database.

`just release 1.0.0` creates and pushes the annotated tag `v1.0.0`; the release
workflow checks it against `Cargo.toml` and `CHANGELOG.md`, builds four targets,
smoke-tests native binaries and publishes one tarball per target containing
`mtr-hk`, `mtr-hk-mcp`, `mtr-hk-mcp-http` and `LICENSE`, plus `SHA256SUMS`.
`scripts/install.sh` verifies the checksum and installs all three binaries, with
a `cargo build --release --locked --bins` fallback.

## Development

```bash
just                 # fmt check, clippy with -D warnings, tests
just skill-check     # validate both SKILL.md files and shell syntax
just worker-check    # type-check the pure-WASM Worker
cargo build --release --locked --bins
scripts/smoke.sh
python3 scripts/mcp_smoke.py
MTR_HTTP_ADDR=127.0.0.1:8080 ./target/release/mtr-hk-mcp-http &
python3 scripts/mcp_http_smoke.py
```

`cargo test` is offline and covers CSV/HTML/network canonicalization/fares/routes/journey legs/status severity/ferry classification. The smoke scripts need network access.

## Limitations

- **Unofficial.** These are public/observed endpoints, not a supported MTR, ferry operator or Transport Department API contract. They can change or rate-limit.
- **Not safety-critical.** Arrival times, journey times and fares are estimates.
- **Live lift outages are not available.** `mtr_station_facilities` is static open data.
- **Journey times** are based on weekday morning traffic, per MTR's own disclaimer.
- **Airport Express fares** come from the trip planner because the open-data fare matrix excludes AEL.
- **High Speed Rail (West Kowloon)** appears in the line list but is not covered by metro next-train or journey tools.
- **LateCert records** are disruption PDFs kept after service normalises. The live status feed is the authoritative signal for current delays.
- **Ferry live ETA is limited to HKKF and Sun Ferry.** Other operators expose timetables or booking pages, not a public ETA API.
- **Ferry full fares** are the published adult single fare from TD; concessions, monthly passes and resident fares are not exhaustive.
- **Kaito/water-taxi/cross-boundary** services can be seasonal, bookable or subject to immigration/terminal rules.
- **Cloudflare Workers is stateless and POST-only.** The pure V8-isolate Worker exposes the 15 tools over Streamable HTTP JSON at `/mcp`; use the native HTTP binary for legacy SSE. Per-isolate caches are not shared across Cloudflare instances.
- **Remote authentication is a single static API key.** Set `MTR_MCP_API_KEY` to protect `/mcp`; put Cloudflare Access or OAuth in front of a public deployment if you need per-user identity.
- **Traditional/Simplified Chinese** matching is exact for Chinese names; use `mtr_stations` if a name variant is not found.

## License

MIT. MTR data is owned by MTR Corporation Limited. Ferry data is owned by the Transport Department and the respective operators, subject to the data.gov.hk terms of use.
