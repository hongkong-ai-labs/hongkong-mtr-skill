#!/usr/bin/env python3
"""Live CLI smoke checks for mtr-hk. Requires network access."""
from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BIN = os.environ.get("MTR_BIN", str(ROOT / "target" / "release" / "mtr-hk"))


def run(*args: str) -> tuple[int, dict | str]:
    result = subprocess.run(
        [BIN, *args, "--json"],
        capture_output=True,
        text=True,
        timeout=45,
    )
    output = result.stdout.strip()
    try:
        parsed = json.loads(output) if output else ""
    except json.JSONDecodeError:
        parsed = output
    return result.returncode, parsed


def require(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


def check_status() -> str:
    code, data = run("status")
    require(code == 0 and isinstance(data, dict), "status failed")
    require(len(data.get("lines", [])) >= 10, "expected at least 10 lines")
    return f"{len(data['lines'])} lines, overall {data['overall']}"


def check_alerts() -> str:
    code, data = run("alerts", "--content=false")
    require(code == 0 and isinstance(data, dict), "alerts failed")
    return f"{data['count']} items, {data['counts']['record']} record(s)"


def check_next() -> str:
    code, data = run("next", "Tsim Sha Tsui", "--line=TWL")
    require(code == 0 and isinstance(data, dict), "next failed")
    trains = sum(len(d["trains"]) for line in data["lines"] for d in line["directions"])
    require(trains > 0, "expected next trains")
    return f"{trains} predictions"


def check_journey() -> str:
    code, data = run("journey", "Admiralty", "Tung Chung", "--max-routes=2")
    require(code == 0 and isinstance(data, dict), "journey failed")
    require(data["routes"], "expected journey routes")
    return f"{len(data['routes'])} routes, {data['routes'][0]['timeMinutes']} min"


def check_fares() -> str:
    code, data = run("fares", "Admiralty", "Tung Chung")
    require(code == 0 and isinstance(data, dict), "fares failed")
    require(data["fares"]["octopus"]["adult"] is not None, "missing adult fare")
    return f"adult Octopus HK${data['fares']['octopus']['adult']}"


def check_stations() -> str:
    code, data = run("stations", "tsuen", "--limit=3")
    require(code == 0 and isinstance(data, dict), "stations failed")
    require(data["count"] > 0, "expected station matches")
    return f"{data['count']} matches"


def check_lines() -> str:
    code, data = run("lines")
    require(code == 0 and isinstance(data, dict), "lines failed")
    require(data["lightRail"]["stopCount"] >= 50, "expected Light Rail stops")
    return f"{data['count']} lines, {data['lightRail']['routeCount']} LR routes"


def check_facilities() -> str:
    code, data = run("facilities", "Admiralty")
    require(code == 0 and isinstance(data, dict), "facilities failed")
    require(data["categories"], "expected facility categories")
    return f"{len(data['categories'])} categories"


def check_lr_next() -> str:
    code, data = run("lr-next", "Tuen Mun Ferry Pier")
    require(code == 0 and isinstance(data, dict), "lr-next failed")
    return f"{len(data['platforms'])} platforms"


def check_lr_routes() -> str:
    code, data = run("lr-routes", "507")
    require(code == 0 and isinstance(data, dict), "lr-routes failed")
    require(data.get("matched") == "route", "expected route match")
    return f"{len(data['routes'])} directions"


def check_lr_prefix() -> str:
    code, data = run("lr-routes", "51")
    require(code == 0 and isinstance(data, dict), "lr-routes prefix failed")
    require(data.get("matched") == "route", "expected prefix match")
    return f"matched {data['routes'][0]['route']}"


def check_lr_fares() -> str:
    code, data = run("lr-fares", "Tuen Mun Ferry Pier", "Tin King")
    require(code == 0 and isinstance(data, dict), "lr-fares failed")
    require(data["fares"]["octopus"]["adult"] is not None, "missing LR fare")
    return f"adult Octopus HK${data['fares']['octopus']['adult']}"


def check_bus_next() -> str:
    code, data = run("bus-next", "506", "--stop=Tuen Mun Ferry Pier")
    require(code == 0 and isinstance(data, dict), "bus-next failed")
    require(data["stops"], "expected bus stops")
    return f"{len(data['stops'])} stop predictions"


def check_bus_routes() -> str:
    code, data = run("bus-routes", "K12")
    require(code == 0 and isinstance(data, dict), "bus-routes failed")
    require(data["routes"], "expected bus route")
    return f"{len(data['routes'][0]['stops'])} stops"


def check_bus_stop_search() -> str:
    code, data = run("bus-routes", "Melody Garden")
    require(code == 0 and isinstance(data, dict), "bus stop search failed")
    require(data.get("matched") == "stops", "expected stop match")
    return f"{data['routeCount']} route(s)"


def check_ferries() -> str:
    code, data = run("ferries")
    require(code == 0 and isinstance(data, dict), "ferries failed")
    require(len(data["operators"]) >= 16, "expected 16 ferry operators")
    return f"{len(data['operators'])} operators, {data['totalRouteCount']} routes"


def check_ferry_search() -> str:
    code, data = run("ferries", "Cheung Chau")
    require(code == 0 and isinstance(data, dict), "ferry search failed")
    require(data["routes"], "expected ferry routes")
    require(any(route.get("liveEta") for route in data["routes"]), "expected a live ferry route")
    return f"{len(data['routes'])} routes"


def check_hkkf_eta() -> str:
    code, data = run("ferry-eta", "hkkf", "1")
    require(code == 0 and isinstance(data, dict), "HKKF ETA failed")
    require(data.get("live") is True, "expected live HKKF ETA")
    return f"{len(data.get('etas', []))} sailing(s)"


def check_sun_eta() -> str:
    code, data = run("ferry-eta", "sun-ferry", "CECC")
    require(code == 0 and isinstance(data, dict), "Sun Ferry ETA failed")
    require(data.get("live") is True, "expected live Sun Ferry ETA")
    return f"{len(data.get('etas', []))} sailing(s)"


def check_invalid_station() -> str:
    result = subprocess.run(
        [BIN, "next", "Not A Station", "--json"],
        capture_output=True,
        text=True,
        timeout=45,
    )
    require(result.returncode != 0, "invalid station should fail")
    return "invalid input rejected"


CHECKS = [
    ("service status", check_status),
    ("traffic alerts", check_alerts),
    ("next trains", check_next),
    ("journey planner", check_journey),
    ("heavy-rail fares", check_fares),
    ("station search", check_stations),
    ("line list", check_lines),
    ("station facilities", check_facilities),
    ("Light Rail next trains", check_lr_next),
    ("Light Rail routes", check_lr_routes),
    ("Light Rail route prefix", check_lr_prefix),
    ("Light Rail fares", check_lr_fares),
    ("MTR Bus next buses", check_bus_next),
    ("MTR Bus routes", check_bus_routes),
    ("MTR Bus stop discovery", check_bus_stop_search),
    ("ferry operators", check_ferries),
    ("ferry route search", check_ferry_search),
    ("HKKF live ETA", check_hkkf_eta),
    ("Sun Ferry live ETA", check_sun_eta),
    ("invalid station rejection", check_invalid_station),
]


def main() -> int:
    if not Path(BIN).is_file():
        print(f"Binary not found: {BIN}. Run cargo build --release.", file=sys.stderr)
        return 1
    failed = 0
    for name, check in CHECKS:
        try:
            detail = check()
            print(f"PASS  {name} — {detail}")
        except Exception as error:  # noqa: BLE001
            failed += 1
            print(f"FAIL  {name} — {error}", file=sys.stderr)
    if failed:
        print(f"\n{failed}/{len(CHECKS)} live checks failed.", file=sys.stderr)
        return 1
    print(f"\nAll {len(CHECKS)} live checks passed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
