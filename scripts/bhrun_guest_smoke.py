#!/usr/bin/env python3
"""Run a live end-to-end smoke test for `bhrun` guest execution.

Required:
  BROWSER_USE_API_KEY

Optional:
  BU_NAME                   defaults to "bhrun-guest-smoke"
  BU_DAEMON_IMPL            defaults to "rust"
  BU_REMOTE_TIMEOUT_MINUTES defaults to "1"
  BU_GUEST_MODE             defaults to "run-guest", or use "serve-guest"
  BU_GUEST_PATH             override the guest module path
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
os.environ.setdefault("BU_NAME", "bhrun-guest-smoke")

from scripts._admin_cli import (  # noqa: E402
    ensure_daemon,
    list_browsers,
    restart_daemon,
    start_remote_daemon,
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


def start_runner_process(subcommand, extra_args=None):
    cmd, cwd = runner_process_spec()
    return subprocess.Popen(
        cmd + [subcommand] + (extra_args or []),
        cwd=cwd,
        env=os.environ.copy(),
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )


def send_line(proc, payload):
    if proc.stdin is None or proc.stdout is None:
        raise RuntimeError("bhrun process pipes are not available")
    proc.stdin.write(json.dumps(payload) + "\n")
    proc.stdin.flush()
    line = proc.stdout.readline()
    if not line:
        stderr = proc.stderr.read().strip() if proc.stderr is not None else ""
        raise RuntimeError(stderr or "bhrun returned no output")
    return json.loads(line)


def finish_runner_process(proc, timeout_seconds=10):
    if proc.stdin is not None and not proc.stdin.closed:
        proc.stdin.close()
    try:
        proc.wait(timeout=timeout_seconds)
    except subprocess.TimeoutExpired:
        proc.kill()
        raise RuntimeError("bhrun command timed out")
    stderr = proc.stderr.read().strip() if proc.stderr is not None else ""
    if proc.returncode != 0:
        raise RuntimeError(stderr or f"bhrun exited {proc.returncode}")
    return stderr


def main():
    if not os.environ.get("BROWSER_USE_API_KEY"):
        raise SystemExit("BROWSER_USE_API_KEY is required")

    os.environ.setdefault("BU_DAEMON_IMPL", "rust")
    name = os.environ["BU_NAME"]
    timeout = int(os.environ.get("BU_REMOTE_TIMEOUT_MINUTES", "1"))
    guest_mode = os.environ.get("BU_GUEST_MODE", "run-guest")
    guest_path = Path(
        os.environ.get(
            "BU_GUEST_PATH",
            str(REPO / "rust" / "guests" / "navigate_and_read.wat"),
        )
    )

    browser = None
    result = {
        "name": name,
        "daemon_impl": os.environ["BU_DAEMON_IMPL"],
        "guest_mode": guest_mode,
        "guest_path": str(guest_path),
    }
    try:
        browser = start_remote_daemon(name=name, timeout=timeout)
        result["browser_id"] = browser["id"]

        sample_config, _ = run_runner_command("sample-config")
        sample_config["daemon_name"] = name
        sample_config["guest_module"] = str(guest_path)
        sample_config["granted_operations"] = [
            "goto",
            "wait_for_load_event",
            "page_info",
            "js",
        ]
        result["guest_config"] = sample_config

        if guest_mode == "run-guest":
            result["guest_run"], _ = run_runner_command(
                "run-guest",
                sample_config,
                timeout_seconds=15,
                extra_args=[str(guest_path)],
            )
            guest_run = result["guest_run"]
        elif guest_mode == "serve-guest":
            proc = start_runner_process("serve-guest", extra_args=[str(guest_path)])
            try:
                result["guest_ready"] = send_line(
                    proc,
                    {"command": "start", "config": sample_config},
                )
                run_response = send_line(proc, {"command": "run"})
                result["guest_run_response"] = run_response
                result["guest_status"] = send_line(proc, {"command": "status"})
                result["guest_stopped"] = send_line(proc, {"command": "stop"})
                stderr = finish_runner_process(proc, timeout_seconds=10)
                if stderr:
                    result["guest_runner_stderr"] = stderr
            except Exception:
                proc.kill()
                raise

            if result["guest_ready"].get("kind") != "ready":
                raise RuntimeError(f"unexpected serve-guest ready response: {result['guest_ready']!r}")
            if result["guest_status"].get("kind") != "status":
                raise RuntimeError(f"unexpected serve-guest status response: {result['guest_status']!r}")
            if result["guest_stopped"].get("kind") != "stopped":
                raise RuntimeError(f"unexpected serve-guest stop response: {result['guest_stopped']!r}")
            if run_response.get("kind") != "run_result":
                raise RuntimeError(f"unexpected serve-guest run response: {run_response!r}")
            guest_run = run_response.get("result") or {}
            result["guest_run"] = guest_run
        else:
            raise RuntimeError(f"unsupported BU_GUEST_MODE: {guest_mode!r}")

        if not guest_run.get("success"):
            raise RuntimeError(f"guest run failed: {guest_run}")
        if guest_run.get("exit_code") != 0:
            raise RuntimeError(f"unexpected guest exit code: {guest_run.get('exit_code')!r}")

        calls = guest_run.get("calls") or []
        result["guest_operations"] = [call.get("operation") for call in calls]
        if result["guest_operations"] != ["goto", "wait_for_load_event", "page_info", "js"]:
            raise RuntimeError(f"unexpected guest operation sequence: {result['guest_operations']!r}")

        page_info_call = calls[2]
        js_call = calls[3]
        page_info_response = page_info_call.get("response") or {}
        js_response = js_call.get("response")
        expected_url = "https://example.com/?via=bhrun-guest-sample"
        if page_info_response.get("url") != expected_url:
            raise RuntimeError("guest page_info response did not match the expected URL")
        if "Example Domain" not in str(js_response):
            raise RuntimeError(f"guest js response did not match the page title: {js_response!r}")

        result["page_after_guest"], result["page_after_guest_request"] = run_runner_command(
            "page-info",
            {"daemon_name": name},
        )
        if result["page_after_guest"].get("url") != expected_url:
            raise RuntimeError("runner page-info after guest did not match the expected URL")

        result["js_after_guest"], result["js_after_guest_request"] = run_runner_command(
            "js",
            {"daemon_name": name, "expression": "location.href"},
        )
        if result["js_after_guest"] != expected_url:
            raise RuntimeError("runner js after guest did not match the expected URL")
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
