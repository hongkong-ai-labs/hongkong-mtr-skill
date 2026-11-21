use anyhow::Result;
use clap::{ArgAction, Parser, Subcommand};
use serde_json::Value;

use crate::app::App;

#[derive(Debug, Parser)]
#[command(
    name = "mtr-hk",
    version,
    about = "Hong Kong MTR MCP/CLI: status, alerts, trains, journeys, fares and accessibility"
)]
struct Cli {
    /// Print raw JSON instead of the human-readable summary.
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Live MTR line service status.
    Status { line: Option<String> },
    /// MTR Traffic News and disruption notices.
    #[command(alias = "news")]
    Alerts {
        line: Option<String>,
        #[arg(long, default_value = "en", value_parser = ["en", "tc"])]
        lang: String,
        #[arg(long, default_value_t = true, action = ArgAction::Set)]
        content: bool,
    },
    /// Next trains at a heavy-rail station.
    Next {
        station: String,
        #[arg(long)]
        line: Option<String>,
        #[arg(long, value_parser = ["up", "down"])]
        direction: Option<String>,
    },
    /// Plan a heavy-rail journey.
    #[command(alias = "plan")]
    Journey {
        from: String,
        to: String,
        #[arg(long = "max-routes", default_value_t = 3)]
        max_routes: u8,
    },
    /// Heavy-rail fares between two stations.
    Fares { from: String, to: String },
    /// Search MTR stations.
    Stations {
        query: String,
        #[arg(long, default_value_t = 8)]
        limit: u8,
    },
    /// List MTR lines.
    Lines {
        #[arg(long)]
        stations: bool,
    },
    /// Barrier-free facilities at a station.
    Facilities { station: String },
    /// Light Rail next trains at a stop.
    LrNext {
        stop: String,
        #[arg(long)]
        route: Option<String>,
    },
    /// Light Rail routes and stops.
    LrRoutes { query: Option<String> },
    /// Light Rail fares between two stops.
    LrFares { from: String, to: String },
    /// MTR Bus next buses for a route.
    #[command(alias = "bus")]
    BusNext {
        route: String,
        #[arg(long)]
        stop: Option<String>,
    },
    /// MTR Bus routes and stops.
    BusRoutes {
        query: Option<String>,
        #[arg(long)]
        stop: Option<String>,
    },
    /// Hong Kong ferry operators and routes.
    Ferries {
        query: Option<String>,
        #[arg(long)]
        operator: Option<String>,
        #[arg(long = "all")]
        all_routes: bool,
    },
    /// Live ferry ETA for HKKF or Sun Ferry.
    FerryEta {
        operator: String,
        route: String,
        #[arg(long, default_value = "outbound", value_parser = ["outbound", "inbound"])]
        direction: String,
    },
}

#[derive(Clone, Copy)]
enum Output {
    Status,
    Alerts,
    Next,
    Journey,
    Fares,
    Stations,
    Lines,
    Facilities,
    LrNext,
    BusNext,
    Ferries,
    FerryEta,
    Json,
}

async fn execute(cli: Cli) -> Result<()> {
    let app = App::new()?;
    let (output, value) = match cli.command {
        Command::Status { line } => (
            Output::Status,
            serde_json::to_value(crate::status::get_service_status(&app, line.as_deref()).await?)?,
        ),
        Command::Alerts {
            line,
            lang,
            content,
        } => (
            Output::Alerts,
            serde_json::to_value(
                crate::alerts::get_alerts(
                    &app,
                    crate::alerts::AlertOptions {
                        line: line.as_deref(),
                        include_content: content,
                        lang: &lang,
                        active_only: false,
                    },
                )
                .await?,
            )?,
        ),
        Command::Next {
            station,
            line,
            direction,
        } => (
            Output::Next,
            serde_json::to_value(
                crate::next_train::get_next_trains(
                    &app,
                    &station,
                    line.as_deref(),
                    direction.as_deref(),
                )
                .await?,
            )?,
        ),
        Command::Journey {
            from,
            to,
            max_routes,
        } => (
            Output::Journey,
            serde_json::to_value(
                crate::journey::plan_journey(&app, &from, &to, Some(max_routes)).await?,
            )?,
        ),
        Command::Fares { from, to } => (
            Output::Fares,
            crate::services::get_rail_fares(&app, &from, &to).await?,
        ),
        Command::Stations { query, limit } => (
            Output::Stations,
            crate::services::search_stations(&app, &query, limit).await?,
        ),
        Command::Lines { stations } => (
            Output::Lines,
            crate::services::list_lines(&app, stations).await?,
        ),
        Command::Facilities { station } => (
            Output::Facilities,
            crate::services::get_station_facilities(&app, &station).await?,
        ),
        Command::LrNext { stop, route } => (
            Output::LrNext,
            crate::services::get_light_rail_next_trains(&app, &stop, route.as_deref()).await?,
        ),
        Command::LrRoutes { query } => (
            Output::Json,
            crate::services::get_light_rail_routes(&app, query.as_deref()).await?,
        ),
        Command::LrFares { from, to } => (
            Output::Fares,
            crate::services::get_light_rail_fares(&app, &from, &to).await?,
        ),
        Command::BusNext { route, stop } => (
            Output::BusNext,
            crate::services::get_bus_next_trains(&app, &route, stop.as_deref()).await?,
        ),
        Command::BusRoutes { query, stop } => (
            Output::Json,
            crate::services::get_bus_routes(&app, query.as_deref(), stop.as_deref()).await?,
        ),
        Command::Ferries {
            query,
            operator,
            all_routes,
        } => (
            Output::Ferries,
            crate::ferries::get_ferries(&app, query.as_deref(), operator.as_deref(), all_routes)
                .await?,
        ),
        Command::FerryEta {
            operator,
            route,
            direction,
        } => (
            Output::FerryEta,
            crate::ferries::get_ferry_eta(&app, &operator, &route, Some(&direction)).await?,
        ),
    };

    if cli.json {
        println!("{}", serde_json::to_string_pretty(&value)?);
    } else {
        println!("{}", human(output, &value));
    }
    Ok(())
}

fn money(value: &Value) -> String {
    value
        .as_f64()
        .map(|value| format!("HK${value:.2}"))
        .unwrap_or_else(|| "n/a".to_string())
}

fn text(value: &Value) -> &str {
    value.as_str().unwrap_or("")
}

fn lines_label(value: &Value) -> String {
    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(", ")
        })
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "All lines".to_string())
}

fn human(output: Output, data: &Value) -> String {
    match output {
        Output::Status => format_status(data),
        Output::Alerts => format_alerts(data),
        Output::Next => format_next(data),
        Output::Journey => format_journey(data),
        Output::Fares => format_fares(data),
        Output::Stations => format_stations(data),
        Output::Lines => format_lines(data),
        Output::Facilities => format_facilities(data),
        Output::LrNext => format_lr_next(data),
        Output::BusNext => format_bus_next(data),
        Output::Ferries => format_ferries(data),
        Output::FerryEta => format_ferry_eta(data),
        Output::Json => serde_json::to_string_pretty(data).unwrap_or_else(|_| "{}".to_string()),
    }
}

fn format_status(data: &Value) -> String {
    let mut lines = vec![format!(
        "MTR service: {}{}",
        text(&data["overall"]).to_uppercase(),
        data["updatedAtHkt"]
            .as_str()
            .or_else(|| data["updatedAt"].as_str())
            .map(|updated| format!("  (last update {updated} HKT)"))
            .unwrap_or_default()
    )];
    for line in data["lines"].as_array().into_iter().flatten() {
        lines.push(format!(
            "- {} ({}): {}",
            text(&line["nameEn"]),
            text(&line["code"]),
            text(&line["statusLabel"])
        ));
        for alert in line["alerts"].as_array().into_iter().flatten() {
            lines.push(format!("    ! {}", text(&alert["title"])));
        }
    }
    if let Some(alerts) = data["alerts"]
        .as_array()
        .filter(|alerts| !alerts.is_empty())
    {
        lines.push(String::new());
        lines.push("Active network notices:".to_string());
        for alert in alerts {
            lines.push(format!(
                "- [{}] {}",
                lines_label(&alert["lines"]),
                text(&alert["title"])
            ));
        }
    }
    if !text(&data["remark"]).is_empty() {
        lines.push(String::new());
        lines.push(text(&data["remark"]).to_string());
    }
    lines.join("\n")
}

fn format_alerts(data: &Value) -> String {
    let Some(alerts) = data["alerts"]
        .as_array()
        .filter(|alerts| !alerts.is_empty())
    else {
        return "No active MTR traffic notices.".to_string();
    };
    let mut lines = vec![format!("MTR traffic news: {} notice(s)", data["count"])];
    for (index, alert) in alerts.iter().enumerate() {
        lines.push(String::new());
        lines.push(format!(
            "{}. [{}] {}",
            index + 1,
            lines_label(&alert["lines"]),
            text(&alert["title"])
        ));
        let mut meta = format!(
            "   Type: {}{} [{}]",
            text(&alert["type"]),
            if text(&alert["tsiType"]) != "None" {
                format!(" / {}", text(&alert["tsiType"]))
            } else {
                String::new()
            },
            text(&alert["severity"])
        );
        if let Some(issued) = alert["issuedTime"].as_str() {
            meta.push_str(&format!("   Issued: {issued}"));
        }
        if !alert["lineNames"]
            .as_array()
            .is_none_or(|items| items.is_empty())
        {
            meta.push_str(&format!("   Lines: {}", lines_label(&alert["lineNames"])));
        }
        lines.push(meta);
        if !text(&alert["content"]).is_empty() {
            lines.push(format!(
                "   {}",
                text(&alert["content"]).replace('\n', "\n   ")
            ));
        }
        if let Some(url) = alert["detailsUrl"].as_str() {
            lines.push(format!("   Details: {url}"));
        }
    }
    lines.join("\n")
}

fn format_next(data: &Value) -> String {
    let mut lines = vec![format!(
        "{} ({})",
        text(&data["station"]["nameEn"]),
        text(&data["station"]["code"])
    )];
    let mut shown = false;
    for line in data["lines"].as_array().into_iter().flatten() {
        lines.push(String::new());
        lines.push(format!(
            "{}{}",
            text(&line["line"]["nameEn"]),
            if line["delayed"].as_bool().unwrap_or(false) {
                " — DELAYED"
            } else {
                ""
            }
        ));
        for direction in line["directions"].as_array().into_iter().flatten() {
            let trains: Vec<String> = direction["trains"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|train| train["valid"].as_bool().unwrap_or(false))
                .map(|train| {
                    format!(
                        "{} min → {}{}",
                        train["minutesAway"]
                            .as_i64()
                            .map(|minutes| minutes.to_string())
                            .unwrap_or_else(|| "?".to_string()),
                        text(&train["destination"]),
                        train["platform"]
                            .as_str()
                            .map(|platform| format!(" (platform {platform})"))
                            .unwrap_or_default()
                    )
                })
                .collect();
            if !trains.is_empty() {
                shown = true;
                lines.push(format!(
                    "  {}: {}",
                    text(&direction["direction"]),
                    trains.join(" | ")
                ));
            }
        }
    }
    if !shown
        && data["lines"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|line| text(&line["line"]["code"]) == "EAL")
    {
        lines.push(String::new());
        lines.push(
            "Live East Rail next-train data is disabled in MTR's open-data feed. \
             Check the MTR app or the published East Rail timetable."
                .to_string(),
        );
    }
    for warning in data["warnings"].as_array().into_iter().flatten() {
        lines.push(String::new());
        lines.push(format!(
            "Note ({}): {}",
            text(&warning["line"]),
            text(&warning["message"])
        ));
    }
    lines.join("\n")
}

fn format_journey(data: &Value) -> String {
    let mut lines = vec![format!(
        "{} → {}",
        text(&data["from"]["nameEn"]),
        text(&data["to"]["nameEn"])
    )];
    if let Some(first) = data["firstTrain"]["time"].as_str() {
        lines.push(format!(
            "First train {first}, last train {}",
            data["lastTrain"]["time"].as_str().unwrap_or("n/a")
        ));
    }
    for (index, route) in data["routes"].as_array().into_iter().flatten().enumerate() {
        lines.push(String::new());
        lines.push(format!(
            "Route {}: {} min, {} interchange(s), {} Octopus / {} single",
            index + 1,
            route["timeMinutes"],
            route["interchanges"],
            money(&route["fares"]["octopusAdult"]),
            money(&route["fares"]["singleAdult"])
        ));
        for leg in route["legs"].as_array().into_iter().flatten() {
            if text(&leg["type"]) == "walk" {
                lines.push(format!(
                    "  - Walk: {} → {} ({} min){}",
                    text(&leg["from"]),
                    text(&leg["to"]),
                    leg["durationMinutes"],
                    leg["note"]
                        .as_str()
                        .map(|note| format!(" — {note}"))
                        .unwrap_or_default()
                ));
            } else {
                lines.push(format!(
                    "  - {}: {} → {} ({} min){}{}",
                    leg["lineName"]
                        .as_str()
                        .or_else(|| leg["lineCode"].as_str())
                        .unwrap_or(""),
                    text(&leg["from"]),
                    text(&leg["to"]),
                    leg["durationMinutes"],
                    leg["boardPlatform"]
                        .as_str()
                        .map(|platform| format!(", board platform {platform}"))
                        .unwrap_or_default(),
                    leg["towards"]
                        .as_str()
                        .map(|towards| format!(", towards {towards}"))
                        .unwrap_or_default()
                ));
            }
        }
    }
    if let Some(note) = data["note"].as_str() {
        lines.push(String::new());
        lines.push(note.to_string());
    }
    lines.join("\n")
}

fn format_fares(data: &Value) -> String {
    let fares = &data["fares"];
    [
        format!(
            "{} → {} ({})",
            text(&data["from"]["nameEn"]),
            text(&data["to"]["nameEn"]),
            text(&data["mode"])
        ),
        format!(
            "Octopus: adult {}, child {}, elderly {}, student {}",
            money(&fares["octopus"]["adult"]),
            money(&fares["octopus"]["child"]),
            money(&fares["octopus"]["elderly"]),
            money(&fares["octopus"]["student"])
        ),
        format!(
            "Single journey: adult {}, child {}, elderly {}",
            money(&fares["singleJourney"]["adult"]),
            money(&fares["singleJourney"]["child"]),
            money(&fares["singleJourney"]["elderly"])
        ),
        format!("Source: {}", text(&data["source"])),
    ]
    .join("\n")
}

fn format_stations(data: &Value) -> String {
    let Some(stations) = data["stations"]
        .as_array()
        .filter(|stations| !stations.is_empty())
    else {
        return format!("No stations matched \"{}\".", text(&data["query"]));
    };
    let mut lines = vec![format!("Stations matching \"{}\":", text(&data["query"]))];
    for station in stations {
        lines.push(format!(
            "- {} ({}) / {} — {}",
            text(&station["nameEn"]),
            text(&station["code"]),
            text(&station["nameTc"]),
            lines_label(&station["lines"])
        ));
    }
    lines.join("\n")
}

fn format_lines(data: &Value) -> String {
    let mut lines = vec![format!("MTR lines: {}", data["count"])];
    for line in data["lines"].as_array().into_iter().flatten() {
        lines.push(format!(
            "- {} ({}){} {} stations",
            text(&line["nameEn"]),
            text(&line["code"]),
            if line["realTime"].as_bool().unwrap_or(false) {
                ""
            } else {
                " [no real-time feed]"
            },
            if line["stationCount"].is_number() {
                line["stationCount"].to_string()
            } else {
                line["stations"]
                    .as_array()
                    .map(|stations| stations.len().to_string())
                    .unwrap_or_else(|| "0".to_string())
            }
        ));
    }
    lines.join("\n")
}

fn format_facilities(data: &Value) -> String {
    let mut lines = vec![format!(
        "{} ({}) accessibility",
        text(&data["station"]["nameEn"]),
        text(&data["station"]["code"])
    )];
    if data["categories"]
        .as_array()
        .is_none_or(|categories| categories.is_empty())
    {
        lines.push("No facility records found.".to_string());
    }
    for category in data["categories"].as_array().into_iter().flatten() {
        lines.push(String::new());
        lines.push(text(&category["category"]).to_string());
        for facility in category["facilities"].as_array().into_iter().flatten() {
            let exits = facility["items"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|item| item["exitEn"].as_str())
                .filter(|exit| !exit.is_empty())
                .collect::<Vec<_>>()
                .join(", ");
            lines.push(format!(
                "  - {}{}",
                text(&facility["name"]),
                if exits.is_empty() {
                    String::new()
                } else {
                    format!(" ({exits})")
                }
            ));
        }
    }
    lines.push(String::new());
    lines.push(text(&data["note"]).to_string());
    lines.join("\n")
}

fn format_lr_next(data: &Value) -> String {
    let mut lines = vec![format!(
        "{} ({}) Light Rail",
        text(&data["stop"]["nameEn"]),
        text(&data["stop"]["nameTc"])
    )];
    for platform in data["platforms"].as_array().into_iter().flatten() {
        lines.push(String::new());
        lines.push(format!("Platform {}", platform["platformId"]));
        for route in platform["routes"].as_array().into_iter().flatten() {
            lines.push(format!(
                "  - {} → {}: {}{}",
                text(&route["route"]),
                text(&route["destination"]),
                text(&route["timeText"]),
                if route["special"].as_bool().unwrap_or(false) {
                    " (special)"
                } else {
                    ""
                }
            ));
        }
    }
    lines.join("\n")
}

fn format_bus_next(data: &Value) -> String {
    let mut lines = vec![format!("MTR Bus {}", text(&data["route"]))];
    for stop in data["stops"].as_array().into_iter().flatten() {
        lines.push(String::new());
        lines.push(format!(
            "{} ({}) — {}",
            text(&stop["stopNameEn"]),
            text(&stop["stopId"]),
            text(&stop["direction"])
        ));
        if stop["suspended"].as_bool().unwrap_or(false) {
            lines.push("  SUSPENDED".to_string());
        }
        for bus in stop["buses"].as_array().into_iter().flatten() {
            lines.push(format!(
                "  - {}{}{}",
                if text(&bus["departureText"]).is_empty() {
                    "n/a"
                } else {
                    text(&bus["departureText"])
                },
                if bus["delayed"].as_bool().unwrap_or(false) {
                    " (delayed)"
                } else {
                    ""
                },
                if bus["scheduled"].as_bool().unwrap_or(false) {
                    " (scheduled)"
                } else {
                    ""
                }
            ));
        }
    }
    lines.join("\n")
}

fn format_ferries(data: &Value) -> String {
    let routes = data["routes"].as_array().cloned().unwrap_or_default();
    let operators = data["operators"].as_array().cloned().unwrap_or_default();
    let mut lines = if routes.is_empty() {
        vec![format!("Hong Kong ferry operators: {}", operators.len())]
    } else {
        vec![format!("Hong Kong ferry routes: {}", routes.len())]
    };

    if routes.is_empty() {
        for operator in &operators {
            lines.push(format!(
                "- {} ({}) [{}]{}: {} route(s)\n  {}",
                text(&operator["nameEn"]),
                text(&operator["id"]),
                text(&operator["scope"]),
                if operator["liveEta"].as_bool().unwrap_or(false) {
                    " live ETA"
                } else {
                    ""
                },
                operator["routeCount"],
                text(&operator["website"])
            ));
        }
    } else {
        for route in &routes {
            lines.push(format!(
                "- [{}] {} → {}{}{}{}",
                text(&route["operatorId"]),
                text(&route["originEn"]),
                text(&route["destinationEn"]),
                route["journeyMinutes"]
                    .as_i64()
                    .map(|minutes| format!(" ({minutes} min)"))
                    .unwrap_or_default(),
                route["fullFare"]
                    .as_f64()
                    .map(|fare| format!(" HK${fare:.2}"))
                    .unwrap_or_default(),
                if route["liveEta"].as_bool().unwrap_or(false) {
                    " [live ETA]"
                } else {
                    ""
                }
            ));
            if let Some(source) = route["sourceUrl"].as_str() {
                lines.push(format!("  {source}"));
            }
        }
    }

    lines.push(String::new());
    lines.push("Source: Transport Department franchised and licensed ferry routes/fares; operator timetables and ETA APIs.".to_string());
    lines.join("\n")
}

fn format_ferry_eta(data: &Value) -> String {
    let mut lines = vec![format!(
        "{} — {}",
        text(&data["operatorName"]),
        data["route"]["nameEn"].as_str().unwrap_or("")
    )];
    if !data["live"].as_bool().unwrap_or(false) {
        lines.push(text(&data["message"]).to_string());
        if let Some(source) = data["source"].as_str() {
            lines.push(format!("Source: {source}"));
        }
        return lines.join("\n");
    }

    let etas = data["etas"].as_array().cloned().unwrap_or_default();
    if etas.is_empty() {
        lines.push("No upcoming sailings returned.".to_string());
    }
    for eta in etas {
        lines.push(format!(
            "- {}{}{}",
            eta["departTime"].as_str().unwrap_or("-"),
            eta["minutesAway"]
                .as_i64()
                .map(|minutes| format!(" ({minutes} min)"))
                .unwrap_or_default(),
            eta["eta"]
                .as_str()
                .map(|value| format!(" → {value}"))
                .unwrap_or_default()
        ));
    }
    if let Some(source) = data["source"].as_str() {
        lines.push(format!("Source: {source}"));
    }
    lines.join("\n")
}

pub async fn run() -> Result<()> {
    let cli = Cli::parse();
    execute(cli).await
}

pub fn error_and_exit(error: anyhow::Error) -> ! {
    eprintln!("Error: {error}");
    std::process::exit(1);
}

pub async fn run_or_exit() {
    if let Err(error) = run().await {
        error_and_exit(error);
    }
}
