#!/usr/bin/env python3
"""Run a live end-to-end smoke test for `bhrun wait-for-dialog`.

Required:
  BROWSER_USE_API_KEY

Optional:
  BU_NAME                   defaults to "bhrun-dialog-smoke"
  BU_DAEMON_IMPL            defaults to "rust"
  BU_REMOTE_TIMEOUT_MINUTES defaults to "1"
  BU_RUST_RUNNER_BIN        override the bhrun binary path
"""

import json
import os
import subprocess
import sys
import time
import uuid
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO))
os.environ.setdefault("BU_NAME", "bhrun-dialog-smoke")

from scripts._admin_cli import (  # noqa: E402
    ensure_daemon,
    list_browsers,
    restart_daemon,
    start_remote_daemon,
)
from scripts._runner_cli import (  # noqa: E402
    drain_events,
    handle_dialog,
    js,
    new_tab,
    page_info,
    wait_for_load,
)


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


def start_runner_command(subcommand, payload):
    cmd, cwd = runner_process_spec()
    proc = subprocess.Popen(
        cmd + [subcommand],
        cwd=cwd,
        env=os.environ.copy(),
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    assert proc.stdin is not None
    proc.stdin.write(json.dumps(payload))
    proc.stdin.close()
    return proc, payload


def finish_runner_command(proc, timeout_seconds):
    try:
        proc.wait(timeout=timeout_seconds)
    except subprocess.TimeoutExpired:
        proc.kill()
        raise RuntimeError("bhrun command timed out")

    stdout = proc.stdout.read() if proc.stdout is not None else ""
    stderr = proc.stderr.read() if proc.stderr is not None else ""
    if proc.returncode != 0:
        raise RuntimeError((stderr or stdout or f"bhrun exited {proc.returncode}").strip())
    if not stdout.strip():
        raise RuntimeError("bhrun returned empty stdout")
    return json.loads(stdout)


def run_runner_command(subcommand, payload, timeout_seconds=10):
    proc, sent = start_runner_command(subcommand, payload)
    return finish_runner_command(proc, timeout_seconds), sent


def main():
    if not os.environ.get("BROWSER_USE_API_KEY"):
        raise SystemExit("BROWSER_USE_API_KEY is required")

    os.environ.setdefault("BU_DAEMON_IMPL", "rust")
    name = os.environ["BU_NAME"]
    timeout = int(os.environ.get("BU_REMOTE_TIMEOUT_MINUTES", "1"))

    browser = None
    result = {"name": name, "daemon_impl": os.environ["BU_DAEMON_IMPL"]}
    try:
        browser = start_remote_daemon(name=name, timeout=timeout)
        result["browser_id"] = browser["id"]
        result["initial_page"] = page_info()
        result["new_tab_target"] = new_tab("https://example.com/?via=bhrun-dialog-smoke")
        result["loaded"] = wait_for_load()
        result["after_nav"] = page_info()

        current_session_result, current_session_request = run_runner_command(
            "current-session",
            {"daemon_name": name},
            timeout_seconds=10,
        )
        result["current_session_request"] = current_session_request
        result["current_session"] = current_session_result
        result["session_id"] = current_session_result.get("session_id")
        if not result["session_id"]:
            raise RuntimeError("runner did not report a current session")

        result["drained_before_wait"] = len(drain_events())
        token = f"bhrun-dialog-smoke-{uuid.uuid4().hex}"
        wait_proc, wait_payload = start_runner_command(
            "wait-for-dialog",
            {
                "daemon_name": name,
                "session_id": result["session_id"],
                "type": "alert",
                "message": token,
                "timeout_ms": 5000,
                "poll_interval_ms": 100,
            },
        )
        result["wait_request"] = wait_payload
        time.sleep(0.5)
        result["js_result"] = js(f"setTimeout(() => alert({json.dumps(token)}), 50); null")
        wait_result = finish_runner_command(wait_proc, timeout_seconds=10)
        result["wait_result"] = wait_result
        event = wait_result.get("event") or {}
        params = event.get("params") or {}
        if not wait_result.get("matched"):
            raise RuntimeError("wait-for-dialog returned matched=false")
        if event.get("method") != "Page.javascriptDialogOpening":
            raise RuntimeError(f"unexpected event method: {event.get('method')!r}")
        if event.get("session_id") != result["session_id"]:
            raise RuntimeError("dialog event session_id did not match the active session")
        if params.get("type") != "alert":
            raise RuntimeError(f"unexpected dialog type: {params.get('type')!r}")
        if params.get("message") != token:
            raise RuntimeError("dialog message did not match the triggered token")

        result["page_info_with_dialog"] = page_info()
        dialog = result["page_info_with_dialog"].get("dialog")
        if not dialog:
            raise RuntimeError("page_info did not surface the pending dialog")
        if dialog.get("type") != "alert":
            raise RuntimeError(f"unexpected page_info dialog type: {dialog.get('type')!r}")
        if dialog.get("message") != token:
            raise RuntimeError("page_info dialog message did not match the triggered token")

        result["dismiss_result"] = handle_dialog(action="accept")
        time.sleep(0.3)
        result["page_info_after_dismiss"] = page_info()
        if "dialog" in result["page_info_after_dismiss"]:
            raise RuntimeError("dialog was still pending after Page.handleJavaScriptDialog")
        result["token"] = token
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
