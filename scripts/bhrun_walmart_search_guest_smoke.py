#!/usr/bin/env python3
"""Run a smoke for the Rust/Wasm Walmart search guest."""

import json
import os
import re
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO))
os.environ.setdefault("BU_NAME", "bhrun-walmart-search-guest-smoke")

TARGET_URL = "https://www.walmart.com/search?q=laptop"


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
            "failed to build the Rust Walmart guest; ensure the stable wasm target is installed "
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
        summarized.append(
            {
                "operation": call.get("operation"),
                "url": request.get("url"),
                "timeout": request.get("timeout"),
                "response_length": len(response) if isinstance(response, str) else None,
            }
        )
    return summarized


def extract_next_data(html):
    match = re.search(r'<script id="__NEXT_DATA__"[^>]*>(.*?)</script>', html, re.DOTALL)
    if not match:
        raise RuntimeError("Walmart __NEXT_DATA__ was not present in the smoke response")
    return json.loads(match.group(1))


def main():
    name = os.environ["BU_NAME"]
    guest_manifest = REPO / "rust" / "guests" / "rust-walmart-search" / "Cargo.toml"
    default_guest_path = (
        REPO
        / "rust"
        / "guests"
        / "rust-walmart-search"
        / "target"
        / "wasm32-unknown-unknown"
        / "release"
        / "rust_walmart_search_guest.wasm"
    )
    guest_path = Path(os.environ.get("BU_GUEST_PATH", str(default_guest_path)))

    result = {
        "name": name,
        "daemon_impl": os.environ.get("BU_DAEMON_IMPL", "rust"),
        "guest_path": str(guest_path),
        "skill": "domain-skills/walmart/scraping.md",
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
    if call.get("request", {}).get("url") != TARGET_URL:
        raise RuntimeError("Walmart request URL mismatch")

    html = call.get("response") or ""
    if "__NEXT_DATA__" not in html:
        raise RuntimeError("Walmart response did not contain __NEXT_DATA__")
    next_data = extract_next_data(html)
    search_result = next_data["props"]["pageProps"]["initialData"]["searchResult"]
    items = []
    for stack in search_result.get("itemStacks", []):
        items.extend(stack.get("items", []))
    product_items = [item for item in items if item.get("usItemId")]
    first = product_items[0]
    result["search_summary"] = {
        "aggregated_count": search_result.get("aggregatedCount"),
        "max_page": (search_result.get("paginationV2") or {}).get("maxPage"),
        "item_count": len(items),
        "product_item_count": len(product_items),
        "first_item": {
            "usItemId": first.get("usItemId"),
            "name": first.get("name"),
            "price": first.get("price"),
            "canonicalUrl": first.get("canonicalUrl"),
        },
    }

    if (result["search_summary"]["aggregated_count"] or 0) < 1000:
        raise RuntimeError("unexpected Walmart aggregatedCount")
    if result["search_summary"]["product_item_count"] < 20:
        raise RuntimeError("unexpected Walmart product item count")
    if not str(result["search_summary"]["first_item"]["canonicalUrl"]).startswith("/ip/"):
        raise RuntimeError("unexpected Walmart canonicalUrl")

    print(json.dumps(result))


if __name__ == "__main__":
    main()
