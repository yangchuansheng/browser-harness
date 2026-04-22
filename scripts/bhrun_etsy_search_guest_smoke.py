#!/usr/bin/env python3
"""Run a smoke for the Rust/Wasm Etsy search guest.

Required in remote mode:
  BROWSER_USE_API_KEY

Optional:
  BU_NAME                   defaults to "bhrun-etsy-search-guest-smoke"
  BU_BROWSER_MODE           defaults to "remote"; set to "local" to attach via DevToolsActivePort
  BU_DAEMON_IMPL            defaults to "rust"
  BU_REMOTE_TIMEOUT_MINUTES defaults to "1"
  BU_LOCAL_DAEMON_WAIT_SECONDS defaults to "15"
  BU_GUEST_PATH             override the guest module path
  BU_SKIP_GUEST_BUILD       set to "1" to skip the default Rust guest build
  BU_RUST_RUNNER_BIN        override the bhrun binary path
"""

import json
import os
import subprocess
import sys
import time
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO))
os.environ.setdefault("BU_NAME", "bhrun-etsy-search-guest-smoke")

from scripts._admin_cli import (  # noqa: E402
    ensure_daemon,
    list_browsers,
    restart_daemon,
    start_remote_daemon,
)

TARGET_URL_PREFIX = "https://www.etsy.com/search"
DIAGNOSTIC_SCRIPT = r"""
JSON.stringify({
  readyState: document.readyState,
  title: document.title,
  url: location.href,
  bodyTextHead: document.body ? document.body.innerText.slice(0, 500) : null,
  listingCount: document.querySelectorAll('[data-listing-id]').length,
  jsonLdCount: document.querySelectorAll('script[type="application/ld+json"]').length,
  listingSample: Array.from(document.querySelectorAll('[data-listing-id]')).slice(0, 10).map(el => ({
    listingId: el.getAttribute('data-listing-id'),
    href: el.querySelector('a[href*="/listing/"]')?.href || null,
    title: el.querySelector('h3, h2')?.innerText?.trim() || null
  }))
})
"""


def poll_browser_status(browser_id, attempts=10, delay=1.0):
    for _ in range(attempts):
        listing = list_browsers(page_size=20, page_number=1)
        item = next((item for item in listing.get("items", []) if item.get("id") == browser_id), None)
        status = item.get("status") if item else "missing"
        if status != "active":
            return status
        time.sleep(delay)
    return status


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
            "failed to build the Rust Etsy search guest; ensure the stable wasm target is installed "
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


def capture_selector_diagnostics(name):
    raw, request = run_runner_command(
        "js",
        {"daemon_name": name, "expression": DIAGNOSTIC_SCRIPT},
        timeout_seconds=10,
    )
    try:
        return json.loads(raw), request
    except json.JSONDecodeError:
        return {"raw": raw}, request


def main():
    browser_mode = os.environ.get("BU_BROWSER_MODE", "remote").strip().lower() or "remote"
    if browser_mode not in {"remote", "local"}:
        raise SystemExit("BU_BROWSER_MODE must be 'remote' or 'local'")
    if browser_mode == "remote" and not os.environ.get("BROWSER_USE_API_KEY"):
        raise SystemExit("BROWSER_USE_API_KEY is required")

    os.environ.setdefault("BU_DAEMON_IMPL", "rust")
    name = os.environ["BU_NAME"]
    timeout = int(os.environ.get("BU_REMOTE_TIMEOUT_MINUTES", "1"))
    local_wait = float(os.environ.get("BU_LOCAL_DAEMON_WAIT_SECONDS", "15"))
    guest_manifest = REPO / "rust" / "guests" / "rust-etsy-search" / "Cargo.toml"
    default_guest_path = (
        REPO
        / "rust"
        / "guests"
        / "rust-etsy-search"
        / "target"
        / "wasm32-unknown-unknown"
        / "release"
        / "rust_etsy_search_guest.wasm"
    )
    guest_path = Path(os.environ.get("BU_GUEST_PATH", str(default_guest_path)))

    browser = None
    result = {
        "name": name,
        "daemon_impl": os.environ["BU_DAEMON_IMPL"],
        "browser_mode": browser_mode,
        "guest_path": str(guest_path),
        "skill": "domain-skills/etsy/scraping.md",
        "target_url_prefix": TARGET_URL_PREFIX,
    }
    try:
        if os.environ.get("BU_SKIP_GUEST_BUILD") != "1" and guest_path == default_guest_path:
            build_guest_module(guest_manifest)
            result["guest_manifest"] = str(guest_manifest)
            result["guest_build_mode"] = "cargo+stable"

        if browser_mode == "remote":
            browser = start_remote_daemon(name=name, timeout=timeout)
            result["browser_id"] = browser["id"]
        else:
            ensure_daemon(name=name, wait=local_wait)
            result["local_attach"] = "DevToolsActivePort"

        sample_config, _ = run_runner_command("sample-config")
        sample_config["daemon_name"] = name
        sample_config["guest_module"] = str(guest_path)
        sample_config["granted_operations"] = [
            "new_tab",
            "wait_for_load",
            "wait",
            "page_info",
            "js",
        ]
        result["guest_config"] = sample_config

        guest_run, _ = run_runner_command(
            "run-guest",
            sample_config,
            timeout_seconds=40,
            extra_args=[str(guest_path)],
        )
        result["guest_run"] = guest_run
        calls = guest_run.get("calls") or []
        operations = [call.get("operation") for call in calls]
        result["guest_operations"] = operations

        if not guest_run.get("success"):
            new_tab_call = next((call for call in calls if call.get("operation") == "new_tab"), None)
            if new_tab_call is not None:
                result["failed_new_tab_response"] = new_tab_call.get("response")
            try:
                result["page_after_failed_guest"], result["page_after_failed_guest_request"] = (
                    run_runner_command("page-info", {"daemon_name": name})
                )
            except Exception as err:
                result["page_after_failed_guest_error"] = str(err)
            try:
                (
                    result["selector_diagnostics_after_failed_guest"],
                    result["selector_diagnostics_after_failed_guest_request"],
                ) = capture_selector_diagnostics(name)
            except Exception as err:
                result["selector_diagnostics_after_failed_guest_error"] = str(err)
            raise RuntimeError(f"guest run failed: {json.dumps(result, sort_keys=True)}")
        if guest_run.get("exit_code") != 0:
            new_tab_call = next((call for call in calls if call.get("operation") == "new_tab"), None)
            if new_tab_call is not None:
                result["failed_new_tab_response"] = new_tab_call.get("response")
            try:
                result["page_after_failed_guest"], result["page_after_failed_guest_request"] = (
                    run_runner_command("page-info", {"daemon_name": name})
                )
            except Exception as err:
                result["page_after_failed_guest_error"] = str(err)
            try:
                (
                    result["selector_diagnostics_after_failed_guest"],
                    result["selector_diagnostics_after_failed_guest_request"],
                ) = capture_selector_diagnostics(name)
            except Exception as err:
                result["selector_diagnostics_after_failed_guest_error"] = str(err)
            raise RuntimeError(f"unexpected guest exit code: {json.dumps(result, sort_keys=True)}")

        expected_prefix = [
            "new_tab",
            "wait_for_load",
            "wait",
            "page_info",
            "js",
        ]
        if operations[: len(expected_prefix)] != expected_prefix:
            raise RuntimeError(f"unexpected guest operation sequence: {operations!r}")
        retries = operations[len(expected_prefix) :]
        if len(retries) % 2 != 0 or any(
            retries[index : index + 2] != ["wait", "js"] for index in range(0, len(retries), 2)
        ):
            raise RuntimeError(f"unexpected guest retry sequence: {operations!r}")

        new_tab_response = calls[0].get("response") or {}
        wait_response = calls[2].get("response") or {}
        page_response = calls[3].get("response") or {}
        raw_items = calls[-1].get("response")

        if not str(new_tab_response.get("target_id", "")).strip():
            raise RuntimeError(f"guest new_tab result did not include target_id: {new_tab_response!r}")
        if int(wait_response.get("elapsed_ms", 0)) < 3000:
            raise RuntimeError(f"guest hydration wait did not sleep long enough: {wait_response!r}")
        if not str(page_response.get("url", "")).startswith(TARGET_URL_PREFIX):
            raise RuntimeError("guest page_info did not remain on the Etsy search URL")

        items = json.loads(raw_items)
        result["search_count"] = len(items)
        result["search_sample"] = items[:3]
        if len(items) < 24:
            (
                result["selector_diagnostics_after_short_extract"],
                result["selector_diagnostics_after_short_extract_request"],
            ) = capture_selector_diagnostics(name)
            raise RuntimeError(f"guest extracted too few Etsy search rows: {len(items)}")
        first = items[0]
        if not str(first.get("name", "")).strip():
            raise RuntimeError("guest extracted an empty Etsy result name")
        if not str(first.get("url", "")).startswith("https://www.etsy.com/listing/"):
            raise RuntimeError("guest extracted a malformed Etsy listing URL")
        position = first.get("position")
        if position is not None and int(position) <= 0:
            raise RuntimeError("guest extracted a malformed Etsy result position")

        result["page_after_guest"], result["page_after_guest_request"] = run_runner_command(
            "page-info",
            {"daemon_name": name},
        )
        if not str(result["page_after_guest"].get("url", "")).startswith(TARGET_URL_PREFIX):
            raise RuntimeError("runner page-info after guest did not remain on the Etsy search URL")
    finally:
        restart_daemon(name)
        time.sleep(1)
        if browser is not None:
            result["post_shutdown_status"] = poll_browser_status(browser["id"])
        log_path = Path(f"/tmp/bu-{name}.log")
        if log_path.exists():
            lines = log_path.read_text().strip().splitlines()
            result["log_tail"] = lines[-8:]

    print(json.dumps(result))


if __name__ == "__main__":
    main()
