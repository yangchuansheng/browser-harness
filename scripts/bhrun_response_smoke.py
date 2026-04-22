#!/usr/bin/env python3
"""Run a live end-to-end smoke test for `bhrun wait-for-response`.

Optional:
  BU_NAME                   defaults to "bhrun-response-smoke"
  BU_BROWSER_MODE           defaults to "remote"; set to "local" to attach via DevToolsActivePort
  BU_DAEMON_IMPL            defaults to "rust"
  BU_REMOTE_TIMEOUT_MINUTES defaults to "1"
  BU_LOCAL_DAEMON_WAIT_SECONDS defaults to "15"
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
os.environ.setdefault("BU_NAME", "bhrun-response-smoke")

from admin import _browser_use, ensure_daemon, restart_daemon, start_remote_daemon  # noqa: E402
from scripts._runner_cli import goto, new_tab, page_info, wait_for_load  # noqa: E402


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
    browser_mode = os.environ.get("BU_BROWSER_MODE", "remote").strip().lower() or "remote"
    if browser_mode not in {"remote", "local"}:
        raise SystemExit("BU_BROWSER_MODE must be 'remote' or 'local'")
    if browser_mode == "remote" and not os.environ.get("BROWSER_USE_API_KEY"):
        raise SystemExit("BROWSER_USE_API_KEY is required")

    os.environ.setdefault("BU_DAEMON_IMPL", "rust")
    name = os.environ["BU_NAME"]
    timeout = int(os.environ.get("BU_REMOTE_TIMEOUT_MINUTES", "1"))
    local_wait = float(os.environ.get("BU_LOCAL_DAEMON_WAIT_SECONDS", "15"))

    browser = None
    result = {
        "name": name,
        "daemon_impl": os.environ["BU_DAEMON_IMPL"],
        "browser_mode": browser_mode,
    }
    try:
        if browser_mode == "remote":
            browser = start_remote_daemon(name=name, timeout=timeout)
            result["browser_id"] = browser["id"]
        else:
            ensure_daemon(name=name, wait=local_wait)
            result["local_attach"] = "DevToolsActivePort"

        result["initial_page"] = page_info()
        result["new_tab_target"] = new_tab("https://example.com/?via=bhrun-response-smoke")
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

        token = uuid.uuid4().hex
        target_url = f"https://example.com/?via=bhrun-response-smoke&token={token}"
        wait_proc, wait_payload = start_runner_command(
            "wait-for-response",
            {
                "daemon_name": name,
                "session_id": result["session_id"],
                "url": target_url,
                "status": 200,
                "timeout_ms": 5000,
                "poll_interval_ms": 100,
            },
        )
        result["wait_request"] = wait_payload
        time.sleep(0.5)
        result["goto_result"] = goto(target_url)
        wait_result = finish_runner_command(wait_proc, timeout_seconds=15)
        result["wait_result"] = wait_result
        event = wait_result.get("event") or {}
        response = event.get("params", {}).get("response", {})
        if not wait_result.get("matched"):
            raise RuntimeError("wait-for-response returned matched=false")
        if event.get("method") != "Network.responseReceived":
            raise RuntimeError(f"unexpected event method: {event.get('method')!r}")
        if event.get("session_id") != result["session_id"]:
            raise RuntimeError("response event session_id did not match the active session")
        if response.get("url") != target_url:
            raise RuntimeError("response event URL did not match the requested target URL")
        if int(response.get("status", 0)) != 200:
            raise RuntimeError(f"unexpected response status: {response.get('status')!r}")
        result["after_wait_page"] = page_info()
        if result["after_wait_page"].get("url") != target_url:
            raise RuntimeError("page URL did not match the navigation triggered for wait-for-response")
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
