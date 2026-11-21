# MTR data sources and API notes

All endpoints are public or are the same unauthenticated endpoints used by MTR's
own web front-end. They are unofficial and may change. The server overrides each
URL through environment variables (see README).

## 1. Next Train (heavy rail)

```
GET https://rt.data.gov.hk/v1/transport/mtr/getSchedule.php?line=TCL&sta=TSY&lang=en
```

- `line`: `AEL`, `DRL`, `EAL`, `ISL`, `KTL`, `SIL`, `TCL`, `TKL`, `TML`, `TWL`
- `sta`: station code, e.g. `TSY`, `ADM`, `TST`
- `lang`: `en` or `zh` (optional; the server translates destination codes itself)

Response:

```json
{
  "sys_time": "2026-11-21 20:39:51",
  "curr_time": "2026-11-21 20:39:47",
  "data": {
    "TCL-TSY": {
      "UP":   [{ "seq": "1", "dest": "TUC", "plat": "3", "time": "...", "ttnt": "3", "valid": "Y", "source": "-" }],
      "DOWN": [{ "seq": "1", "dest": "HOK", "plat": "4", "time": "...", "ttnt": "6", "valid": "Y", "source": "-" }]
    }
  },
  "isdelay": "N",
  "status": 1,
  "message": "successful"
}
```

- `ttnt` = time to next train in minutes.
- `isdelay` = `Y` when the line is reporting a delay.
- `status` = `0` means the station/line combination is unavailable or suspended;
  `message` explains why.
- Only the documented line/station combinations return data.
- `source` may be `-`, `+` or other operational markers; treat as diagnostic.

Official spec: `https://opendata.mtr.com.hk/doc/Next_Train_API_Spec_v1.7.pdf`

## 2. Light Rail next train

```
GET https://rt.data.gov.hk/v1/transport/mtr/lrt/getSchedule?station_id=1&with_special=1
```

- `station_id` is the numeric Light Rail **Stop ID** from
  `light_rail_routes_and_stops.csv` (e.g. Tuen Mun Ferry Pier = 1).
- `with_special=1` includes special routes.

Response: `platform_list[].route_list[]` with `route_no`, `dest_en`, `dest_ch`,
`time_en`, `time_ch`, `arrival_departure` (`A`/`D`), `special`, `train_length`,
`routeRemarkEng2`, `routeRemarkChi2`, `additionalInfo1`. The server exposes the
remark in both languages and marks special routes.

Light Rail route/stop metadata comes from
`light_rail_routes_and_stops.csv`: 11 route codes, 22 directions, 68 unique
stops. Stop IDs in that CSV are the `station_id` values used by this API.

Official spec: `https://opendata.mtr.com.hk/doc/LR_Next_Train_API_Spec_v1.1.pdf`

## 3. MTR Bus / feeder bus

```
POST https://rt.data.gov.hk/v1/transport/mtr/bus/getSchedule
Content-Type: application/json

{ "language": "en", "routeName": "506" }
```

Response: `busStop[]`, each with `busStopId`, `isSuspended`, and `bus[]`.
Each bus has `busId`, `departureTimeText`, `departureTimeInSecond`,
`isDelayed`, `isScheduled` and `lineRef`. Top-level fields include
`routeStatus`, `routeStatusColour`, `routeStatusRemarkTitle`,
`routeStatusRemarkContent`, `routeStatusTime`, `appRefreshTimeInSecond` and
`footerRemarks`; the server exposes these as `serviceStatus` on the next-bus
tool. Route status codes are passed through rather than interpreted.

- `routeName` accepts the route number (`506`, `K12`). Route variants use their
  reference id (`506-1`).
- The response does **not** include stop names; join `busStopId` to
  `mtr_bus_stops.csv` (`STATION_ID`).
- `lineRef` identifies the direction variant (e.g. `506_TMFP`, `506_SL`).

Official spec: `https://opendata.mtr.com.hk/doc/MTR_BUS_API_Spec_v1.14.pdf`

## 4. Service status (traffic news)

```
GET https://tnews.mtr.com.hk/alert/ryg_line_status.xml
```

```xml
<ryg_status>
  <lastBuildDate>Sat, 21 Nov 2026 12:00:04 GMT</lastBuildDate>
  <refreshInterval>180000</refreshInterval>
  <line>
    <line_code>TWL</line_code>
    <url_tc></url_tc><url_en></url_en>
    <status>green</status>
  </line>
  ...
</ryg_status>
```

Status colours: `green` (normal), `yellow` (delay), `red` (disruption),
`grey` (non-service hours). The legend also defines `pink` (service delay or
disruption) and `typhoon` (Tropical Cyclone Warning Signal in force). The server
reports `overall` as `normal`, `delay`, `disruption`, `special`, `typhoon` or
`non-service`, and includes a code-level summary of affected lines.

Legend: `https://tnews.mtr.com.hk/alert/legend_remarks_v2.xml`
Line metadata/colours: `https://tnews.mtr.com.hk/alert/lineconfig.json`

This is the same XML consumed by MTR's service-status page
(`https://www.mtr.com.hk/en/customer/main/service_status.html`).

## 5. Traffic notices and LateCert PDFs

```
GET https://tnews.mtr.com.hk/alert/alert.json
GET https://tnews.mtr.com.hk/alert/latecert.json
```

`alert.json` entries have `newsType` (`TSI` or `LateCert`), `tsiType`, `status`,
`id`, `lineCodes` (null = network-wide), `alertTitle`, `alertTitleTc`,
`alertContent` (HTML), `alertContentTc`, `cause`, `show_map`,
`show_shuttlebus`, `show_alttrans`. The server classifies each item:

- `disruption` — disruption wording or map/shuttle/alternative-route flags
- `delay` — delay wording
- `notice` — planned or general service notice
- `record` — LateCert disruption notification PDF kept for reference

`LateCert` = train service disruption notification PDF. MTR keeps these in the
feed after service returns to normal, so they are treated as records rather than
live disruptions. The live signal is the status XML.

`latecert.json` maps ids to PDF URLs under
`https://tnews.mtr.com.hk/alert/latecert/...`.

## 6. Trip planner and station base info

MTR's web trip planner calls an API discovered in
`https://tnews.mtr.com.hk/alert/config.json`:

```
GET https://proxy001.api.mtr.com.hk/ttp-api/v2/api/StationBaseInfo/HRStations/
Header: api-key: <key from config.json>
```

Returns `lines[]` (ID, alias, bilingual name, colour, ordered station IDs) and
`stations[]` (ID, alias, bilingual + simplified name, line IDs, coordinate).

```
GET https://proxy001.api.mtr.com.hk/ttp-api/v2/api/Path/HRRoutes/?o=44&d=47
Header: api-key: <key from config.json>
```

- `o` / `d` are numeric station IDs.
- Response: `errorCode`, `errorMsg`, `firstTrain`, `lastTrain`,
  `firstLastTrainRemark`, `routes[]`.
- Each route: `time` (minutes), `interchangeStationsNo`, `walkingTime`,
  `fares` (`standardClass`, `firstClass`, `AEL`), and `path[]`.
- `path[]` entries: `ID` (station id), `lineID`, `linkType`
  (`RIDE`, `INTERCHANGE`, `WALKINTERCHANGE`, `END`), cumulative `time` in
  minutes, `platform`, `towards`, `linkText`.

Notes:

- The key is a public client key embedded in MTR's own front-end config. It is
  not a secret in the security sense, but it can be rotated. Override with
  `MTR_TTP_API_KEY`.
- The header name is `api-key` (not `Ocp-Apim-Subscription-Key`).
- Airport Express fares appear only in this API, not in the heavy-rail fare CSV.
- Journey times are based on weekday morning traffic.
- High Speed Rail (station 888) appears in the station base info but is not a
  metro line for next-train or journey purposes.

## 7. Ferries

### Route and fare directory

```
GET https://static.data.gov.hk/td/routes-fares-xml/ROUTE_FERRY.xml
```

The Transport Department's franchised and licensed ferry route/fare dataset. At the time of writing it contains 58 routes across `INNER` (Victoria Harbour), `OUTLYING` (islands), `KAITO` (licensed small ferries) and `WTX` (water taxi). Fields used by the server:

- `ROUTE_ID`, `DISTRICT`, `ROUTE_NAMEE`, `ROUTE_NAMEC`, `ROUTE_NAMES`
- `JOURNEY_TIME` (minutes), `FULL_FARE` (adult full fare, HKD)
- `LOC_START_NAMEE`, `LOC_END_NAMEE` and Chinese equivalents
- `HYPERLINK_E` — TD service-detail page
- `SERVICE_MODE` — ordinary/fast/water-taxi marker

The XML does not contain an operator field (all rows use `COMPANY_CODE=FERRY`). The server classifies each route by route id/name into a curated operator set. The operator directory also includes cross-boundary and dangerous-goods operators that are not in this local-route XML.

### Operator coverage

The server exposes 16 operator records:

- Local scheduled: Star Ferry, Sun Ferry, HKKF, Discovery Bay Transportation, Park Island Transport, Fortune Ferry, Chuen Kee, Coral Sea, Tsui Wah, Peng Chau Kai To.
- Kaito/water taxi: Kaito / licensed small ferries, Hong Kong Water Taxi.
- Cross-boundary: TurboJET, Cotai Water Jet, Chu Kong Passenger Transport.
- Dangerous-goods-only: Hongkong & Yaumati Ferry.

Route-to-operator classification is intentionally conservative. Kaito routes without a confirmed single operator are labelled `kaito` rather than guessed. Cross-boundary operators have no local routes in the TD XML; their operator records carry notes and official websites.

### Live ferry ETA APIs

**HKKF**

```
GET https://www.hkkfeta.com/opendata/route/
GET https://www.hkkfeta.com/opendata/eta/{route_id}/{direction}
```

- Route ids: `1` Central–Sok Kwu Wan, `2` Central–Yung Shue Wan, `3` Central–Peng Chau, `4` Peng Chau–Hei Ling Chau.
- Direction: `outbound` or `inbound`.
- Response: `data[]` with `route_id`, `direction`, `date`, `session_time` and `ETA` (RFC 3339 with +08:00).
- The server returns `minutesAway` by comparing `ETA` with the current time.

**Sun Ferry**

```
GET https://www.sunferry.com.hk/eta/?route={route_code}
```

- Route codes include `CECC` (Central–Cheung Chau), `CEMW` (Central–Mui Wo), `NPHH` (North Point–Hung Hom), `NPKC` (North Point–Kowloon City), and `II...` inter-island variants.
- Response: `generated_timestamp` and `data[]` with `routecode`, `route_en`, `route_tc`, `vesselcode`, `depart_time` (HH:MM), `eta` (HH:MM or null), `rmk_en`, `rmk_tc`.
- `minutesAway` uses the `eta` clock time when present, otherwise `depart_time`, resolving the next occurrence in HKT.

**Fortune Ferry**

The operator advertises an ETA endpoint at `hongkongwatertaxi.com.hk/eta/?route=...`, but it is frequently unavailable/Cloudflare-protected and the upstream community integration currently falls back to timetables. This server does not claim live Fortune data.

**Star Ferry / Park Island / Discovery Bay / kaito / cross-boundary**

No public live ETA API is integrated. The tool returns the operator website and TD timetable source link.

### Ferry caching and failure modes

- Ferry route/fare XML is static-ish and cached for 6 hours.
- HKKF/Sun Ferry ETAs are fetched per request, not cached.
- If the TD ferry XML fails, the ferry tools return an upstream error; no hard-coded route list is substituted.
- ETAs are estimates; weather, vessel changes and special schedules can alter sailings.

## 8. Open-data CSVs

Base: `https://opendata.mtr.com.hk/data/`

| File | Used for |
|---|---|
| `mtr_lines_and_stations.csv` | Fallback line/station catalogue; line station order |
| `mtr_lines_fares.csv` | Heavy-rail fare matrix (excludes AEL) |
| `light_rail_routes_and_stops.csv` | Light Rail routes, stops and stop IDs |
| `light_rail_fares.csv` | Light Rail fare matrix |
| `mtr_bus_routes.csv` | Bus route ids, names, directions, variants |
| `mtr_bus_stops.csv` | Bus stop names, ids, sequence, coordinates |
| `mtr_bus_fares.csv` | Bus fares by route (all MTR Bus and feeder routes) |
| `barrier_free_facilities.csv` | Per-station facility keys and exit labels |
| `barrier_free_facility_category.csv` | Maps facility keys to readable categories |

Datasets:
- Next train: https://data.gov.hk/en-data/dataset/mtr-data2-nexttrain-data
- Routes/fares/accessibility: https://data.gov.hk/en-data/dataset/mtr-data-routes-fares-barrier-free-facilities
- Bus ETA: https://data.gov.hk/en-data/dataset/mtr-mtr_bus-mtr-bus-eta-data

The barrier-free CSV uses keys such as `AJ3` (lift – unpaid area), `AJ8`
(lift – paid area), `MJ1` (wide gates), `VJ1` (tactile guide paths). The
category CSV supplies the human-readable names. Facility data is static, not a
live lift outage feed.

## 9. Caching in this server

| Data | TTL |
|---|---|
| Station/line network, fares, LR, bus, facilities | 6 hours |
| Ferry routes/fares | 6 hours |
| Service status XML | 20 seconds |
| Alerts / LateCert | 60 seconds |
| Journey plans | 60 seconds |

Next-train, Light Rail and bus predictions are not cached. Concurrent identical
loads are de-duplicated.

## 10. Failure modes

- `429` or `5xx` from `rt.data.gov.hk`: back off; the server retries once.
- Station/line combination invalid: API returns `status: 0` with a message.
- Suspended station: `message` names the suspended station.
- Trip planner returns `errorCode != "0"`; surface its `errorMsg`.
- If the private station API is down, the server falls back to the open-data CSV
  for station/line resolution (but journey planning needs the private API).
- LateCert PDF URLs can be missing; the server omits `detailsUrl` if unknown.

## 11. Legal / etiquette

These are unofficial integrations. Respect MTR's terms of use and the
data.gov.hk terms. Do not bulk-scrape, do not present results as official MTR
information, and do not use the data for safety-critical decisions.
