# MTR + ferry CLI reference

The `mtr-hk` CLI shares the same Rust service layer as the MCP server.

## Install

Requires Rust 1.89+.

```bash
cargo install --path .
```

This installs:

- `mtr-hk` — CLI
- `mtr-hk-mcp` — MCP stdio server
- `mtr-hk-mcp-http` — MCP Streamable HTTP/SSE server

Verify with:

```bash
mtr-hk status
```

If the command is not found, add Cargo's bin directory to `PATH`:

```bash
export PATH="$HOME/.cargo/bin:$PATH"
```

For a development build:

```bash
cargo build --release
./target/release/mtr-hk status
MTR_HTTP_ADDR=127.0.0.1:8080 ./target/release/mtr-hk-mcp-http
```

From the skill, the wrapper resolves `mtr-hk` on `PATH`, then the repository's release/debug binary:

```bash
skills/mtr/scripts/mtr <command> [arguments] [flags]
```

Add `--json` to any command for machine-readable output. Without it, the CLI prints a compact human-readable summary.

## Commands

### `status [line]`

```bash
mtr-hk status
mtr-hk status TWL
mtr-hk status "Tuen Ma Line"
```

Shows overall condition, every line's status, last-update time in HKT and any current notices. JSON includes a `summary` with delayed/disrupted/typhoon/special/non-service line codes. Optional line can be a code or bilingual name.

### `alerts [line]`

```bash
mtr-hk alerts
mtr-hk alerts TML
mtr-hk alerts --lang=tc
mtr-hk alerts --content=false
```

Lists traffic news and LateCert disruption records. Each item has a severity (`disruption`, `delay`, `notice`, `record`), affected line codes and line names. `--lang=tc` returns Traditional Chinese titles/content. `--content=false` gives titles only.

### `next <station>`

```bash
mtr-hk next Admiralty
mtr-hk next "Tsim Sha Tsui" --line=TWL
mtr-hk next 金鐘 --line=ISL --direction=up
```

Shows up to four trains per direction, with destination, platform and minutes. `--line` restricts to one line; `--direction` accepts `up` or `down`.

### `journey <from> <to>`

```bash
mtr-hk journey Admiralty "Tung Chung"
mtr-hk journey HOK AEL --max-routes=2
```

Returns estimated minutes, interchange count, fares and step-by-step legs. `--max-routes` is 1–6 (default 3).

### `fares <from> <to>`

```bash
mtr-hk fares "Hong Kong" "Tung Chung"
mtr-hk fares ADM TUC
```

Heavy-rail fare matrix: adult/child/elderly/student/JoyYou/PWD, Octopus and single journey. Airport Express falls back to the trip planner.

### `stations <query>`

```bash
mtr-hk stations tsuen
mtr-hk stations 金鐘
mtr-hk stations TST
```

Searches names in English, Traditional/Simplified Chinese and codes. Use this to disambiguate before other commands.

### `lines [--stations]`

```bash
mtr-hk lines
mtr-hk lines --stations
```

Lists heavy-rail lines, codes, colours, station counts and real-time availability, plus a Light Rail summary (11 routes, 22 directions, 68 stops). With `--stations`, includes the ordered heavy-rail station list and all Light Rail route stops.

### `facilities <station>`

```bash
mtr-hk facilities Admiralty
mtr-hk facilities "Tung Chung"
```

Static barrier-free facilities grouped by category, with exit labels where MTR publishes them. Not live lift outage status.

### `lr-next <stop>`

```bash
mtr-hk lr-next "Tuen Mun Ferry Pier"
mtr-hk lr-next 屯門碼頭 --route=507
mtr-hk lr-next 1 --route=507
```

Light Rail predictions by platform. The stop can be a name, code or numeric stop ID. Optional `--route` filters one route number.

### `lr-routes [query]`

```bash
mtr-hk lr-routes
mtr-hk lr-routes 507
mtr-hk lr-routes "Tin King"
mtr-hk lr-routes 1
```

No query: all Light Rail routes/directions. Route number: ordered stops with origin/destination names. Partial route numbers work when unambiguous (`51` → 751); ambiguous prefixes list the matching routes. Stop name, stop code or numeric stop ID: routes serving that stop, with direction names and stop sequence.

### `lr-fares <from> <to>`

```bash
mtr-hk lr-fares "Tuen Mun Ferry Pier" "Tin King"
```

Octopus and single-journey Light Rail fares.

### `bus-next <route> [--stop=name]`

```bash
mtr-hk bus-next 506
mtr-hk bus-next 506 --stop="Tuen Mun Ferry Pier"
mtr-hk bus-next K12
```

MTR Bus/feeder bus predictions. Without `--stop`, all stops on the route are shown; with a stop filter, only matching stops. Output includes route-level `serviceStatus` remarks, per-stop suspension flags and `suspendedStopCount`.

### `bus-routes [query] [--stop=name]`

```bash
mtr-hk bus-routes
mtr-hk bus-routes K12
mtr-hk bus-routes 506 --stop="Tuen Mun"
```

No query: route list with fares. Route query: variants, direction names, fares and stops in both directions. Stop-name query: every route serving that stop (e.g. `Melody Garden` → 506). Optional stop filter narrows stops within a route.

### `ferries [query]`

```bash
mtr-hk ferries
mtr-hk ferries "Cheung Chau"
mtr-hk ferries "Central"
mtr-hk ferries --operator=sun-ferry
mtr-hk ferries --all
```

No query lists all 16 operators with scope, website, live-ETA availability and route count. A query searches route names, piers, islands, districts and operator names. `--operator` filters to one operator id (`star-ferry`, `sun-ferry`, `hkkf`, `discovery-bay`, `park-island`, `fortune-ferry`, `chuen-kee`, `coral-sea`, `tsui-wah`, `peng-chau-kaito`, `hk-yaumati`, `water-taxi`, `kaito`, `turbojet`, `cotai-water-jet`, `chu-kong`). `--all` includes every matching route when no query is supplied.

Route fields include `operatorId`, `operatorName`, `nameEn`, `nameTc`, `district`, `journeyMinutes`, `originEn`, `destinationEn`, `fullFare`, `sourceUrl`, `liveEta` and, for live operators, `etaRoute`.

### `ferry-eta <operator> <route>`

```bash
mtr-hk ferry-eta sun-ferry CECC
mtr-hk ferry-eta hkkf 1 --direction=outbound
mtr-hk ferry-eta star-ferry 7030
```

Live next-sailing predictions for HKKF and Sun Ferry. HKKF accepts `--direction=outbound` or `inbound`; default `outbound`. For Star Ferry, Fortune Ferry, Park Island, Discovery Bay, kaito and cross-boundary operators, the command returns `live: false` with the official timetable/source link because they do not expose a public ETA API.

Sun Ferry route codes include `CECC` (Central–Cheung Chau), `CEMW` (Central–Mui Wo), `NPHH` (North Point–Hung Hom) and `NPKC` (North Point–Kowloon City). HKKF route ids are `1` Central–Sok Kwu Wan, `2` Central–Yung Shue Wan, `3` Central–Peng Chau and `4` Peng Chau–Hei Ling Chau.

## Global flags

| Flag | Meaning |
|---|---|
| `--json` | Print raw JSON instead of the human summary |
| `--lang=en\|tc` | Language for alert titles/content (default `en`) |
| `--help` | Show usage |

## Exit codes

- `0`: success
- `1`: input error, ambiguous station/line/operator, or upstream failure

Errors go to stderr. The CLI never prints partial/guessed schedules.

## Examples

```bash
# Morning commute check
mtr-hk status
mtr-hk alerts
mtr-hk next "Hong Kong" --line=TCL

# Airport run
mtr-hk journey "Hong Kong" Airport --max-routes=2
mtr-hk fares "Hong Kong" Airport

# Light Rail + feeder bus
mtr-hk lr-next "Tuen Mun Ferry Pier" --route=507
mtr-hk bus-next 506 --stop="Tuen Mun Ferry Pier"

# Ferries
mtr-hk ferries
mtr-hk ferries "Cheung Chau"
mtr-hk ferry-eta sun-ferry CECC
mtr-hk ferry-eta hkkf 1

# Accessibility
mtr-hk facilities "Hong Kong"
```
