---
name: mtr-hk
description: Query Hong Kong MTR live service status, delays, traffic notices, next trains, journey plans, fares, Light Rail, MTR Bus, station accessibility, and Hong Kong ferry operators/routes with live HKKF and Sun Ferry ETAs. Use when a task involves Hong Kong transport routes, arrival times, disruptions, fares, or journey planning.
---

# Hong Kong MTR and ferries

Use this skill for Hong Kong MTR questions (service status, delays, notices, next trains, journey planning, fares, Light Rail, MTR Bus, accessibility) and for Hong Kong ferry questions (operators, routes, piers, fares, timetables and live ferry ETAs).

MTR data comes from MTR's public open-data APIs plus the traffic-news and trip-planner endpoints used by MTR's own website/app. Ferry route/fare data comes from the Transport Department's franchised and licensed ferry dataset; live ferry ETAs come from HKKF and Sun Ferry. These integrations are unofficial and can change. Never invent an arrival time, fare or service status.

## Install (Rust)

The server and CLI are Rust binaries. Node/npm are **not** required.

Prerequisites:

- Rust 1.88 or newer, including `cargo` (`rustup` is the easiest install: <https://rustup.rs>).
- `~/.cargo/bin` on `PATH` (the Rust installer normally adds it).

From the repository root:

```bash
cargo install --path .
```

This installs three binaries into `~/.cargo/bin`:

```bash
mtr-hk-mcp        # MCP stdio server
mtr-hk-mcp-http   # MCP Streamable HTTP/SSE server
mtr-hk            # human/JSON CLI
```

Verify:

```bash
mtr-hk status
mtr-hk ferries
command -v mtr-hk-mcp
```

If `mtr-hk` is not found, add Cargo's bin directory to your shell profile:

```bash
export PATH="$HOME/.cargo/bin:$PATH"
```

For development without installing:

```bash
cargo build --release
./target/release/mtr-hk status
./target/release/mtr-hk-mcp
```

### Register the MCP server

Use the installed binary in Claude Code, Claude Desktop, Cursor or any stdio MCP client:

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

The repository's `.mcp.json` already uses this command. After `cargo install --path .`, approve or restart the server in your client. If your client does not inherit your shell `PATH`, use the absolute path, e.g. `"command": "/Users/you/.cargo/bin/mtr-hk-mcp"`.

### Remote HTTP/SSE MCP

For a remote or container deployment, run:

```bash
MTR_HTTP_ADDR=0.0.0.0:8080 mtr-hk-mcp-http
```

and point an HTTP MCP client at `http://localhost:8080/mcp`. The endpoint supports Streamable HTTP and SSE-framed responses. It has no built-in authentication, so protect it before public exposure.

The repository also contains a `Dockerfile`, `compose.yaml`, and a Cloudflare Worker + Container deployment in `cloudflare/`. See `README.md` for the container commands.

### Install the agent skill

Link this directory into your client's skills folder:

```bash
mkdir -p ~/.claude/skills
ln -sfn "$PWD/skills/mtr" ~/.claude/skills/mtr
```

The repository also has `.claude/skills/mtr` linked to `../../skills/mtr` for project-local discovery.

### CLI fallback

If MCP tools are unavailable, use the installed CLI or the repository wrapper:

```bash
mtr-hk status
mtr-hk next "Tsim Sha Tsui" --line=TWL
mtr-hk journey Admiralty "Tung Chung"
mtr-hk ferries "Cheung Chau"
skills/mtr/scripts/mtr status
```

Add `--json` to any command for raw structured output. The wrapper looks for `mtr-hk` on `PATH`, then `target/release/mtr-hk`, then `target/debug/mtr-hk`.

## Choose the right tool

| Question | MCP tool | CLI fallback |
|---|---|---|
| Is the MTR running normally? Any delays? | `mtr_status` | `mtr-hk status` |
| What do the current traffic notices say? | `mtr_alerts` | `mtr-hk alerts` |
| Next trains at a station | `mtr_next_trains` | `mtr-hk next "<station>"` |
| Best route between two MTR stations | `mtr_plan_journey` | `mtr-hk journey "<from>" "<to>"` |
| Fare between two MTR stations | `mtr_fares` | `mtr-hk fares "<from>" "<to>"` |
| Find a station code / disambiguate a name | `mtr_stations` | `mtr-hk stations "<query>"` |
| List lines and stations, including Light Rail | `mtr_lines` | `mtr-hk lines [--stations]` |
| Lifts, ramps, accessible toilets, step-free exits | `mtr_station_facilities` | `mtr-hk facilities "<station>"` |
| Light Rail arrivals | `mtr_light_rail_next_trains` | `mtr-hk lr-next "<stop>"` |
| Light Rail routes and stops | `mtr_light_rail_routes` | `mtr-hk lr-routes [query]` |
| Light Rail fares | `mtr_light_rail_fares` | `mtr-hk lr-fares "<from>" "<to>"` |
| MTR Bus / feeder bus arrivals | `mtr_bus_next_trains` | `mtr-hk bus-next <route>` |
| MTR Bus routes and stops | `mtr_bus_routes` | `mtr-hk bus-routes [query]` |
| Ferry operators, routes, piers and fares | `mtr_ferries` | `mtr-hk ferries [query]` |
| Live ferry ETA | `mtr_ferry_eta` | `mtr-hk ferry-eta <operator> <route>` |

## Workflows

### 1. MTR service status and delays

1. Call `mtr_status` with no line for a network-wide answer, or with a line code/name when the user named one.
2. Read `overall` plus `summary`: `delayedLines`, `disruptedLines`, `typhoonLines`, `specialLines`, `nonServiceLines` and `noticeCount`. `overall` can be `normal`, `delay`, `disruption`, `special`, `typhoon` or `non-service`.
3. If any line is `yellow`/`red`/`pink`/`typhoon`, call `mtr_alerts` for that line to get the cause and advice.
4. Answer with: overall condition, affected line(s), what passengers should expect, and any alternative arrangement mentioned in the notice.
5. Include the feed's last-updated time (`updatedAtHkt`). Do not describe a line as delayed if `mtr_status` says normal.

`mtr_alerts` classifies each item as `disruption`, `delay`, `notice` or `record`, and includes `lineNames`. Planned notices (early last trains, engineering works) are `notice`. Historical LateCert disruption PDFs are `record`, not live disruptions. Current status comes from `mtr_status`; a LateCert against a green line is a past event.

For a typhoon, say a Tropical Cyclone Warning Signal is affecting service and report the affected lines/arrangements from the notice; do not reduce it to a generic delay.

### 2. Next trains

1. If the station name is ambiguous, call `mtr_stations` first.
2. Call `mtr_next_trains` with the station; add `line` for interchanges.
3. Present the next 2–3 trains per direction with destination, minutes away and platform. Mention the line name, not just the code.
4. If the result says `delayed: true`, mention it and check `mtr_status`/`mtr_alerts`.
5. A station can be served by several lines; report each separately.

Direction is expressed by destination. `UP`/`DOWN` alone is not useful to a passenger.

### 3. Journey planning

1. Call `mtr_plan_journey` with origin and destination names or codes.
2. Present the fastest route first: total minutes, number of interchanges, fare, then the legs in order.
3. For each leg give the line, boarding station and platform, direction/towards, and alighting station. Include walking interchanges (e.g. Central ↔ Hong Kong) explicitly.
4. Offer an alternative route only when it is meaningfully different (different line, fewer interchanges, or much cheaper).
5. State that the journey time is a weekday-morning estimate and that actual times vary.

Use `mtr_fares` for a standalone fare question; it gives the full passenger-category matrix from open data.

### 4. Light Rail and MTR Bus

- Light Rail stops and heavy-rail stations can share names (e.g. Tuen Mun). Use the Light Rail tools only when the user means Light Rail; say which mode you are reporting.
- Light Rail status is part of `mtr_status` (code `LR`); `mtr_alerts LR` returns any LR-related notices. Use `mtr_light_rail_routes` for all 11 routes, 22 directions and 68 stops, and accept partial route numbers (e.g. `51` finds 751).
- For Light Rail next trains, give route number, destination, platform and minutes. Mark special routes and include any route remark. Stops accept a name, three-letter code or numeric stop ID.
- `mtr_light_rail_routes` accepts a route number (partial when unambiguous), stop name, stop code or numeric stop ID.
- For MTR Bus, give route, stop, direction and next departures. `mtr_bus_next_trains` also returns route-level service status/remarks, `suspendedStopCount` and per-stop suspension flags.
- `mtr_bus_routes` accepts a route number/name **or a stop name**. Use a stop name (e.g. “Melody Garden”) to discover every MTR Bus route serving it.
- MTR Bus route variants can run in different directions; report the direction name (origin → destination), not just the `O`/`I` code.
- If the user has not named a stop, show the first relevant stops and ask which one they mean rather than dumping every stop.

### 5. Ferries

`mtr_ferries` is the ferry directory. It covers:

- **Local scheduled ferries:** Star Ferry, Sun Ferry, Hong Kong & Kowloon Ferry (HKKF), Discovery Bay Transportation, Park Island Transport, Fortune Ferry, Chuen Kee, Coral Sea, Tsui Wah, Peng Chau Kai To.
- **Kaito and water taxi:** licensed small ferries and Hong Kong Water Taxi.
- **Cross-boundary:** TurboJET, Cotai Water Jet and Chu Kong Passenger Transport (Macao and Guangdong ports).
- **Dangerous-goods-only:** Hongkong & Yaumati Ferry (not a normal passenger service).

Typical calls:

```text
mtr_ferries
mtr_ferries query="Cheung Chau"
mtr_ferries operator="hkkf"
mtr_ferries query="Macao"
mtr_ferries include_routes=true
```

What to report:

1. Operator name, route (origin → destination), journey time and full adult fare when present.
2. Whether live ETA is available. **Only HKKF and Sun Ferry publish live ETA APIs in this integration.** Star Ferry, Fortune Ferry, Park Island, Discovery Bay, kaito and cross-boundary operators do not; give their timetable/source link instead.
3. For HKKF/Sun Ferry, call `mtr_ferry_eta` with the operator id and route code. HKKF accepts `outbound`/`inbound` direction. Present departure time and minutes away.
4. The Transport Department's `fullFare` is the published full adult fare, not every concession/round-trip/monthly fare. For exact concession or resident fares, use the operator's fare table.
5. Kaito/water-taxi services can be seasonal, bookable or demand-responsive. Say so rather than implying a turn-up-and-go commuter service.
6. Cross-boundary ferry questions involve immigration/terminal requirements; route data is a directory, not a booking or border-advice service.

Examples:

```bash
mtr-hk ferries
mtr-hk ferries "Cheung Chau"
mtr-hk ferries --operator=sun-ferry
mtr-hk ferry-eta sun-ferry CECC
mtr-hk ferry-eta hkkf 1 --direction=outbound
```

### 6. Accessibility

`mtr_station_facilities` returns static barrier-free facility records: lifts, ramps, wide gates, accessible toilets, tactile guidance, induction loops and step-free exit information. It is not live lift/escalator outage data. If the user needs live lift status, say it is not available from these feeds and suggest checking MTR Mobile or station staff. Ferry accessibility is not consistently published; direct the user to the operator.

## Handling input and errors

- Accept English, Traditional Chinese, Simplified Chinese and station codes (e.g. `ADM`, `金鐘`, `Admiralty`).
- If a tool returns an error with suggestions, retry once with the suggested station/line/operator. If still ambiguous, ask the user which one they mean.
- If an upstream call fails, report the failure plainly. Do not fall back to guessed schedules.
- Do not call every tool for a simple question. One status call is enough for "is the MTR normal?".

## Data freshness and limits

- Service status: refreshed roughly every 3 minutes upstream (`refreshSeconds` says 180); cached about 20 seconds. It exposes both GMT and HKT timestamps.
- Next trains / Light Rail / Bus: near real-time; do not cache in your answer beyond the current request.
- Traffic notices: cached about 1 minute; `counts` breaks down active/disruption/delay/notice/record items.
- Fares, routes, stops and facilities: static open data, refreshed every few hours.
- Ferry routes/fares: Transport Department dataset, cached 6 hours. Live HKKF/Sun Ferry ETAs are fetched per request.
- Journey times are estimates based on weekday morning traffic.
- Fares are in HKD; distinguish Octopus from single-journey tickets.
- Airport Express fares come from the trip planner because the open-data fare matrix excludes AEL.
- High Speed Rail (West Kowloon) is listed but is not covered by the metro next-train or journey tools.
- Ferry route coverage is the TD's franchised/licensed dataset plus a curated operator directory. Private charters, hotel shuttles and unscheduled boats are out of scope.
- These endpoints are unofficial. Do not scrape aggressively, and do not present the data as an official MTR, ferry operator or Transport Department service.

## Response style

- Lead with the answer, then the detail.
- Use Hong Kong local time and `minutes` for imminent arrivals; use clock times only when the source provides them.
- For disruptions, be factual: line, status, cause if given, expected impact, alternative if given.
- Mention the data timestamp when reporting service status.
- Keep fare answers concise: adult Octopus and adult single journey are usually enough unless the user asks about concessions. For ferries, say the published full fare and point to the operator for concessions.
- State the mode clearly: heavy rail, Light Rail, MTR Bus or ferry.

## References

- `references/api-notes.md` — endpoints, parameters, response shapes, ferry sources and limitations.
- `references/cli.md` — full CLI reference and examples.
