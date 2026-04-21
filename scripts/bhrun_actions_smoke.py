#!/usr/bin/env python3
"""Run a live end-to-end smoke test for runner-owned `page-info`, `goto`, and `js`.

Required:
  BROWSER_USE_API_KEY

Optional:
  BU_NAME                   defaults to "bhrun-actions-smoke"
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
os.environ.setdefault("BU_NAME", "bhrun-actions-smoke")

from admin import _browser_use, restart_daemon, start_remote_daemon  # noqa: E402


def poll_browser_status(browser_id, attempts=10, delay=1.0):
    for _ in range(attempts):
        listing = _browser_use("/browsers?pageSize=20&pageNumber=1", "GET")
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


def run_runner_command(subcommand, payload=None, timeout_seconds=10):
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
    stdin_text = "" if payload is None else json.dumps(payload)
    stdout, stderr = proc.communicate(stdin_text, timeout=timeout_seconds)
    if proc.returncode != 0:
        raise RuntimeError((stderr or stdout or f"bhrun exited {proc.returncode}").strip())
    if not stdout.strip():
        raise RuntimeError("bhrun returned empty stdout")
    return json.loads(stdout), payload


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

        result["initial_page"], result["initial_page_request"] = run_runner_command(
            "page-info",
            {"daemon_name": name},
        )
        current_session_result, current_session_request = run_runner_command(
            "current-session",
            {"daemon_name": name},
        )
        result["current_session_request"] = current_session_request
        result["current_session"] = current_session_result
        result["session_id"] = current_session_result.get("session_id")
        if not result["session_id"]:
            raise RuntimeError("runner did not report a current session")

        token = uuid.uuid4().hex
        target_url = f"https://example.com/?via=bhrun-actions-smoke&token={token}"
        wait_proc, wait_payload = start_runner_command(
            "wait-for-load-event",
            {
                "daemon_name": name,
                "session_id": result["session_id"],
                "timeout_ms": 5000,
                "poll_interval_ms": 100,
            },
        )
        result["wait_request"] = wait_payload
        time.sleep(0.5)
        result["goto_result"], result["goto_request"] = run_runner_command(
            "goto",
            {"daemon_name": name, "url": target_url},
        )
        wait_result = finish_runner_command(wait_proc, timeout_seconds=10)
        result["wait_result"] = wait_result
        if not wait_result.get("matched"):
            raise RuntimeError("wait-for-load-event returned matched=false")
        event = wait_result.get("event") or {}
        if event.get("method") != "Page.loadEventFired":
            raise RuntimeError(f"unexpected load event method: {event.get('method')!r}")
        if event.get("session_id") != result["session_id"]:
            raise RuntimeError("load event session_id did not match the active session")

        result["page_after_goto"], result["page_after_goto_request"] = run_runner_command(
            "page-info",
            {"daemon_name": name},
        )
        if result["page_after_goto"].get("url") != target_url:
            raise RuntimeError("page-info URL did not match the navigation target")

        result["js_href"], result["js_href_request"] = run_runner_command(
            "js",
            {"daemon_name": name, "expression": "location.href"},
        )
        if result["js_href"] != target_url:
            raise RuntimeError("js(location.href) did not match the navigation target")

        result["js_title"], result["js_title_request"] = run_runner_command(
            "js",
            {"daemon_name": name, "expression": "document.title"},
        )
        if "Example Domain" not in str(result["js_title"]):
            raise RuntimeError(f"unexpected document.title from js(): {result['js_title']!r}")

        result["target_url"] = target_url
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
