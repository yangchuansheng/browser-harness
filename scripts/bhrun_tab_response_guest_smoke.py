#!/usr/bin/env python3
"""Run a live end-to-end smoke test for the tab/session/response Rust guest.

Required:
  BROWSER_USE_API_KEY

Optional:
  BU_NAME                   defaults to "bhrun-tab-response-guest-smoke"
  BU_DAEMON_IMPL            defaults to "rust"
  BU_REMOTE_TIMEOUT_MINUTES defaults to "1"
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
os.environ.setdefault("BU_NAME", "bhrun-tab-response-guest-smoke")

from scripts._admin_cli import (  # noqa: E402
    ensure_daemon,
    list_browsers,
    restart_daemon,
    start_remote_daemon,
)

TARGET_URL = "https://example.com/?via=bhrun-tab-response-guest-smoke"


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
            "failed to build the Rust tab-response guest; ensure the stable wasm target is installed "
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


def main():
    if not os.environ.get("BROWSER_USE_API_KEY"):
        raise SystemExit("BROWSER_USE_API_KEY is required")

    os.environ.setdefault("BU_DAEMON_IMPL", "rust")
    name = os.environ["BU_NAME"]
    timeout = int(os.environ.get("BU_REMOTE_TIMEOUT_MINUTES", "1"))
    guest_manifest = REPO / "rust" / "guests" / "rust-tab-response-workflow" / "Cargo.toml"
    default_guest_path = (
        REPO
        / "rust"
        / "guests"
        / "rust-tab-response-workflow"
        / "target"
        / "wasm32-unknown-unknown"
        / "release"
        / "rust_tab_response_workflow_guest.wasm"
    )
    guest_path = Path(os.environ.get("BU_GUEST_PATH", str(default_guest_path)))

    browser = None
    result = {
        "name": name,
        "daemon_impl": os.environ["BU_DAEMON_IMPL"],
        "guest_path": str(guest_path),
        "target_url": TARGET_URL,
    }
    try:
        if os.environ.get("BU_SKIP_GUEST_BUILD") != "1" and guest_path == default_guest_path:
            build_guest_module(guest_manifest)
            result["guest_manifest"] = str(guest_manifest)
            result["guest_build_mode"] = "cargo+stable"

        browser = start_remote_daemon(name=name, timeout=timeout)
        result["browser_id"] = browser["id"]

        sample_config, _ = run_runner_command("sample-config")
        sample_config["daemon_name"] = name
        sample_config["guest_module"] = str(guest_path)
        sample_config["granted_operations"] = [
            "current_tab",
            "list_tabs",
            "new_tab",
            "switch_tab",
            "current_session",
            "goto",
            "wait_for_response",
            "page_info",
            "js",
        ]
        result["guest_config"] = sample_config

        guest_run, _ = run_runner_command(
            "run-guest",
            sample_config,
            timeout_seconds=20,
            extra_args=[str(guest_path)],
        )
        result["guest_run"] = guest_run

        if not guest_run.get("success"):
            raise RuntimeError(f"guest run failed: {guest_run!r}")
        if guest_run.get("exit_code") != 0:
            raise RuntimeError(f"unexpected guest exit code: {guest_run.get('exit_code')!r}")

        calls = guest_run.get("calls") or []
        operations = [call.get("operation") for call in calls]
        result["guest_operations"] = operations
        expected_operations = [
            "current_tab",
            "list_tabs",
            "new_tab",
            "current_tab",
            "current_session",
            "js",
            "switch_tab",
            "current_session",
            "current_tab",
            "switch_tab",
            "current_session",
            "current_tab",
            "goto",
            "wait_for_response",
            "page_info",
            "js",
            "list_tabs",
        ]
        if operations != expected_operations:
            raise RuntimeError(f"unexpected guest operation sequence: {operations!r}")

        initial_tab = (calls[0].get("response") or {})
        initial_tabs = calls[1].get("response") or []
        new_tab_response = (calls[2].get("response") or {})
        current_after_new = (calls[3].get("response") or {})
        new_session = (calls[4].get("response") or {})
        blank_href = calls[5].get("response")
        switch_back = (calls[6].get("response") or {})
        session_after_switch_back = (calls[7].get("response") or {})
        current_after_switch_back = (calls[8].get("response") or {})
        switch_forward = (calls[9].get("response") or {})
        session_after_switch_forward = (calls[10].get("response") or {})
        current_after_switch_forward = (calls[11].get("response") or {})
        wait_result = calls[13].get("response") or {}
        final_page = calls[14].get("response") or {}
        final_href = calls[15].get("response")
        final_tabs = calls[16].get("response") or []

        initial_target_id = initial_tab.get("targetId")
        new_target_id = new_tab_response.get("target_id")
        active_session_id = session_after_switch_forward.get("session_id")
        if not initial_target_id:
            raise RuntimeError("guest initial current_tab result did not include targetId")
        if not new_target_id:
            raise RuntimeError("guest new_tab result did not include target_id")
        if new_target_id == initial_target_id:
            raise RuntimeError("guest new_tab target_id matched the initial target")
        if current_after_new.get("targetId") != new_target_id:
            raise RuntimeError("guest current_tab after new_tab did not move to the new target")
        if not new_session.get("session_id"):
            raise RuntimeError("guest current_session after new_tab was empty")
        if blank_href != "about:blank":
            raise RuntimeError(f"guest js after new_tab did not report about:blank: {blank_href!r}")
        if session_after_switch_back.get("session_id") != switch_back.get("session_id"):
            raise RuntimeError("guest current_session did not match switch_tab after switching back")
        if current_after_switch_back.get("targetId") != initial_target_id:
            raise RuntimeError("guest current_tab did not return to the initial target")
        if session_after_switch_forward.get("session_id") != switch_forward.get("session_id"):
            raise RuntimeError("guest current_session did not match switch_tab after switching forward")
        if current_after_switch_forward.get("targetId") != new_target_id:
            raise RuntimeError("guest current_tab did not move back to the new target")
        if not active_session_id:
            raise RuntimeError("guest active session after switch-forward was empty")

        event = wait_result.get("event") or {}
        response = event.get("params", {}).get("response", {})
        if not wait_result.get("matched"):
            raise RuntimeError("guest wait_for_response returned matched=false")
        if event.get("method") != "Network.responseReceived":
            raise RuntimeError(f"unexpected wait_for_response method: {event.get('method')!r}")
        if event.get("session_id") != active_session_id:
            raise RuntimeError("guest wait_for_response event did not match the active session")
        if response.get("url") != TARGET_URL:
            raise RuntimeError("guest wait_for_response URL did not match the target URL")
        if int(response.get("status", 0)) != 200:
            raise RuntimeError(f"unexpected wait_for_response status: {response.get('status')!r}")
        if final_page.get("url") != TARGET_URL:
            raise RuntimeError("guest final page_info did not match the target URL")
        if final_href != TARGET_URL:
            raise RuntimeError("guest final js href did not match the target URL")
        if not any(tab.get("targetId") == new_target_id for tab in final_tabs):
            raise RuntimeError("guest final list_tabs result lost the new target")
        if len(final_tabs) < len(initial_tabs) + 1:
            raise RuntimeError("guest final list_tabs result did not grow after creating a new tab")

        result["page_after_guest"], result["page_after_guest_request"] = run_runner_command(
            "page-info",
            {"daemon_name": name},
        )
        if result["page_after_guest"].get("url") != TARGET_URL:
            raise RuntimeError("runner page-info after guest did not match the target URL")

        result["current_tab_after_guest"], result["current_tab_after_guest_request"] = run_runner_command(
            "current-tab",
            {"daemon_name": name},
        )
        if result["current_tab_after_guest"].get("targetId") != new_target_id:
            raise RuntimeError("runner current-tab after guest did not stay on the new target")

        result["current_session_after_guest"], result["current_session_after_guest_request"] = (
            run_runner_command("current-session", {"daemon_name": name})
        )
        if result["current_session_after_guest"].get("session_id") != active_session_id:
            raise RuntimeError("runner current-session after guest did not match the guest session")

        result["tabs_after_guest"], result["tabs_after_guest_request"] = run_runner_command(
            "list-tabs",
            {"daemon_name": name},
        )
        target_ids = {tab.get("targetId") for tab in result["tabs_after_guest"]}
        if new_target_id not in target_ids:
            raise RuntimeError("runner list-tabs after guest did not include the new tab target")
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
