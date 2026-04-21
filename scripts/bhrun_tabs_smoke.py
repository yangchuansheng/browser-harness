#!/usr/bin/env python3
"""Run a live end-to-end smoke test for runner-owned tab/session commands.

Required:
  BROWSER_USE_API_KEY

Optional:
  BU_NAME                   defaults to "bhrun-tabs-smoke"
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
os.environ.setdefault("BU_NAME", "bhrun-tabs-smoke")

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

        result["initial_current_tab"], result["initial_current_tab_request"] = run_runner_command(
            "current-tab",
            {"daemon_name": name},
        )
        result["initial_tabs"], result["initial_tabs_request"] = run_runner_command(
            "list-tabs",
            {"daemon_name": name},
        )
        initial_target_id = result["initial_current_tab"]["targetId"]

        token = uuid.uuid4().hex
        target_url = f"https://example.com/?via=bhrun-tabs-smoke&token={token}"
        result["new_tab"], result["new_tab_request"] = run_runner_command(
            "new-tab",
            {"daemon_name": name, "url": target_url},
        )
        new_target_id = result["new_tab"]["target_id"]

        result["current_after_new"], result["current_after_new_request"] = run_runner_command(
            "current-tab",
            {"daemon_name": name},
        )
        if result["current_after_new"]["targetId"] != new_target_id:
            raise RuntimeError("current-tab did not move to the new tab target")

        result["page_after_new"], result["page_after_new_request"] = run_runner_command(
            "page-info",
            {"daemon_name": name},
        )
        if result["page_after_new"].get("url") != target_url:
            raise RuntimeError("page-info did not report the new tab URL")

        result["tabs_after_new"], result["tabs_after_new_request"] = run_runner_command(
            "list-tabs",
            {"daemon_name": name},
        )
        target_ids = {tab["targetId"] for tab in result["tabs_after_new"]}
        if initial_target_id not in target_ids or new_target_id not in target_ids:
            raise RuntimeError("list-tabs did not include both the initial and new targets")

        result["switch_back"], result["switch_back_request"] = run_runner_command(
            "switch-tab",
            {"daemon_name": name, "target_id": initial_target_id},
        )
        result["session_after_switch_back"], result["session_after_switch_back_request"] = (
            run_runner_command("current-session", {"daemon_name": name})
        )
        if result["session_after_switch_back"].get("session_id") != result["switch_back"]["session_id"]:
            raise RuntimeError("current-session did not match switch-tab result after switching back")

        result["current_after_switch_back"], result["current_after_switch_back_request"] = (
            run_runner_command("current-tab", {"daemon_name": name})
        )
        if result["current_after_switch_back"]["targetId"] != initial_target_id:
            raise RuntimeError("current-tab did not move back to the initial target")

        result["page_after_switch_back"], result["page_after_switch_back_request"] = (
            run_runner_command("page-info", {"daemon_name": name})
        )
        if result["page_after_switch_back"].get("url") != result["initial_current_tab"].get("url"):
            raise RuntimeError("page-info after switch-back did not match the initial tab URL")

        result["switch_forward"], result["switch_forward_request"] = run_runner_command(
            "switch-tab",
            {"daemon_name": name, "target_id": new_target_id},
        )
        result["session_after_switch_forward"], result["session_after_switch_forward_request"] = (
            run_runner_command("current-session", {"daemon_name": name})
        )
        if result["session_after_switch_forward"].get("session_id") != result["switch_forward"]["session_id"]:
            raise RuntimeError("current-session did not match switch-tab result after switching forward")

        result["current_after_switch_forward"], result["current_after_switch_forward_request"] = (
            run_runner_command("current-tab", {"daemon_name": name})
        )
        if result["current_after_switch_forward"]["targetId"] != new_target_id:
            raise RuntimeError("current-tab did not move back to the new target")

        result["page_after_switch_forward"], result["page_after_switch_forward_request"] = (
            run_runner_command("page-info", {"daemon_name": name})
        )
        if result["page_after_switch_forward"].get("url") != target_url:
            raise RuntimeError("page-info after switch-forward did not match the new tab URL")

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
