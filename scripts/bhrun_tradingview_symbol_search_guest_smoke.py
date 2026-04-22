#!/usr/bin/env python3
"""Run a smoke for the Rust/Wasm TradingView symbol-search guest."""

import json
import os
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO))
os.environ.setdefault("BU_NAME", "bhrun-tradingview-symbol-search-guest-smoke")

TARGET_URL = (
    "https://symbol-search.tradingview.com/symbol_search/v3/"
    "?text=AAPL&hl=1&exchange=NASDAQ&lang=en&search_type=stock&domain=production"
)


def runner_process_spec():
    if custom := os.environ.get("BU_RUST_RUNNER_BIN"):
        return [custom], str(REPO)
    return ["cargo", "run", "--quiet", "--bin", "bhrun", "--"], str(REPO / "rust")


def build_guest_module(guest_manifest):
    proc = subprocess.run(
        [
            "cargo",
            "+stable",
            "build",
            "--offline",
            "--release",
            "--target",
            "wasm32-unknown-unknown",
            "--manifest-path",
            str(guest_manifest),
        ],
        cwd=REPO,
        env=os.environ.copy(),
        text=True,
        capture_output=True,
    )
    if proc.returncode != 0:
        detail = (proc.stderr or proc.stdout or "guest build failed").strip()
        raise RuntimeError(
            "failed to build the Rust TradingView guest; ensure the stable wasm target is installed "
            "via `rustup target add --toolchain stable-x86_64-unknown-linux-gnu wasm32-unknown-unknown`"
            f"\n{detail}"
        )


def run_runner_command(subcommand, payload=None, timeout_seconds=10, extra_args=None):
    cmd, cwd = runner_process_spec()
    proc = subprocess.Popen(
        cmd + [subcommand] + (extra_args or []),
        cwd=cwd,
        env=os.environ.copy(),
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    stdin_text = "" if payload is None else json.dumps(payload)
    stdout, stderr = proc.communicate(stdin_text, timeout=timeout_seconds)
    if proc.returncode != 0:
        raise RuntimeError((stderr or stdout or f"bhrun exited {proc.returncode}").strip())
    if not stdout.strip():
        raise RuntimeError("bhrun returned empty stdout")
    return json.loads(stdout), payload


def summarize_calls(calls):
    summarized = []
    for call in calls:
        request = call.get("request") or {}
        response = call.get("response")
        entry = {
            "operation": call.get("operation"),
            "url": request.get("url"),
            "timeout": request.get("timeout"),
            "response_length": len(response) if isinstance(response, str) else None,
        }
        if request.get("headers"):
            entry["headers"] = request["headers"]
        summarized.append(entry)
    return summarized


def main():
    name = os.environ["BU_NAME"]
    guest_manifest = REPO / "rust" / "guests" / "rust-tradingview-symbol-search" / "Cargo.toml"
    default_guest_path = (
        REPO
        / "rust"
        / "guests"
        / "rust-tradingview-symbol-search"
        / "target"
        / "wasm32-unknown-unknown"
        / "release"
        / "rust_tradingview_symbol_search_guest.wasm"
    )
    guest_path = Path(os.environ.get("BU_GUEST_PATH", str(default_guest_path)))

    result = {
        "name": name,
        "daemon_impl": os.environ.get("BU_DAEMON_IMPL", "rust"),
        "guest_path": str(guest_path),
        "skill": "domain-skills/tradingview/scraping.md",
        "mode": "http_only",
        "target_url": TARGET_URL,
    }

    if os.environ.get("BU_SKIP_GUEST_BUILD") != "1" and guest_path == default_guest_path:
        build_guest_module(guest_manifest)
        result["guest_manifest"] = str(guest_manifest)
        result["guest_build_mode"] = "cargo+stable"

    sample_config, _ = run_runner_command("sample-config")
    sample_config["daemon_name"] = name
    sample_config["guest_module"] = str(guest_path)
    sample_config["granted_operations"] = ["http_get"]
    sample_config["allow_http"] = True
    result["guest_config"] = sample_config

    guest_run, _ = run_runner_command(
        "run-guest",
        sample_config,
        timeout_seconds=30,
        extra_args=[str(guest_path)],
    )
    calls = guest_run.get("calls") or []
    operations = [call.get("operation") for call in calls]
    result["guest_run"] = {
        "exit_code": guest_run.get("exit_code"),
        "success": guest_run.get("success"),
        "trap": guest_run.get("trap"),
    }
    result["guest_operations"] = operations
    result["guest_calls"] = summarize_calls(calls)

    if not guest_run.get("success") or guest_run.get("exit_code") != 0:
        raise RuntimeError(f"guest run failed: {json.dumps(result, sort_keys=True)}")
    if operations != ["http_get"]:
        raise RuntimeError(f"unexpected guest operation sequence: {operations!r}")

    call = calls[0]
    request = call.get("request") or {}
    if request.get("url") != TARGET_URL:
        raise RuntimeError("TradingView request URL mismatch")
    if (request.get("headers") or {}).get("Origin") != "https://www.tradingview.com":
        raise RuntimeError("TradingView Origin header mismatch")

    payload = json.loads(call.get("response") or "{}")
    symbols = payload.get("symbols") or []
    first = symbols[0]
    result["search_summary"] = {
        "symbols_remaining": payload.get("symbols_remaining"),
        "symbol_count": len(symbols),
        "first_symbol": {
            "symbol": first.get("symbol"),
            "description": first.get("description"),
            "type": first.get("type"),
            "exchange": first.get("exchange"),
            "isin": first.get("isin"),
            "currency_code": first.get("currency_code"),
            "is_primary_listing": first.get("is_primary_listing"),
        },
    }

    if result["search_summary"]["symbol_count"] < 1:
        raise RuntimeError("unexpected TradingView symbol count")
    if result["search_summary"]["first_symbol"]["description"] != "Apple Inc.":
        raise RuntimeError("unexpected TradingView first symbol description")
    if result["search_summary"]["first_symbol"]["exchange"] != "NASDAQ":
        raise RuntimeError("unexpected TradingView first symbol exchange")

    print(json.dumps(result))


if __name__ == "__main__":
    main()
