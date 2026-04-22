#!/usr/bin/env python3
"""Run a live end-to-end smoke test for `bhrun serve-guest` with a real browser.

Required:
  BROWSER_USE_API_KEY

Optional:
  BU_NAME                   defaults to "bhrun-persistent-guest-remote-smoke"
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
os.environ.setdefault("BU_NAME", "bhrun-persistent-guest-remote-smoke")

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
            "failed to build the Rust persistent guest; ensure the stable wasm target is installed "
            "via `rustup target add --toolchain stable-x86_64-unknown-linux-gnu wasm32-unknown-unknown`"
            f"\n{detail}"
        )


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


def run_runner_ndjson_command(subcommand, payload, timeout_seconds=10):
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
    stdin_text = json.dumps(payload)
    stdout, stderr = proc.communicate(stdin_text, timeout=timeout_seconds)
    if proc.returncode != 0:
        raise RuntimeError((stderr or stdout or f"bhrun exited {proc.returncode}").strip())
    if not stdout.strip():
        raise RuntimeError("bhrun returned empty stdout")
    return [json.loads(line) for line in stdout.splitlines() if line.strip()], payload


def start_serve_guest_process(guest_path):
    cmd, cwd = runner_process_spec()
    return subprocess.Popen(
        cmd + ["serve-guest", str(guest_path)],
        cwd=cwd,
        env=os.environ.copy(),
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )


def send_line(proc, payload):
    if proc.stdin is None or proc.stdout is None:
        raise RuntimeError("serve-guest process pipes are not available")
    proc.stdin.write(json.dumps(payload) + "\n")
    proc.stdin.flush()
    line = proc.stdout.readline()
    if not line:
        stderr = ""
        if proc.stderr is not None:
            stderr = proc.stderr.read().strip()
        raise RuntimeError(stderr or "serve-guest returned no output")
    return json.loads(line)


def finish_serve_guest(proc, timeout_seconds=10):
    if proc.stdin is not None and not proc.stdin.closed:
        proc.stdin.close()
    try:
        proc.wait(timeout=timeout_seconds)
    except subprocess.TimeoutExpired:
        proc.kill()
        raise RuntimeError("serve-guest command timed out")
    stderr = proc.stderr.read().strip() if proc.stderr is not None else ""
    if proc.returncode != 0:
        raise RuntimeError(stderr or f"serve-guest exited {proc.returncode}")
    return stderr


def main():
    if not os.environ.get("BROWSER_USE_API_KEY"):
        raise SystemExit("BROWSER_USE_API_KEY is required")

    os.environ.setdefault("BU_DAEMON_IMPL", "rust")
    name = os.environ["BU_NAME"]
    timeout = int(os.environ.get("BU_REMOTE_TIMEOUT_MINUTES", "1"))
    guest_manifest = REPO / "rust" / "guests" / "rust-persistent-browser-state" / "Cargo.toml"
    default_guest_path = (
        REPO
        / "rust"
        / "guests"
        / "rust-persistent-browser-state"
        / "target"
        / "wasm32-unknown-unknown"
        / "release"
        / "rust_persistent_browser_state_guest.wasm"
    )
    guest_path = Path(os.environ.get("BU_GUEST_PATH", str(default_guest_path)))
    target_url = "https://example.com/?via=bhrun-serve-guest-remote-smoke"

    browser = None
    serve_guest = None
    result = {
        "name": name,
        "daemon_impl": os.environ["BU_DAEMON_IMPL"],
        "guest_path": str(guest_path),
        "target_url": target_url,
    }
    try:
        if os.environ.get("BU_SKIP_GUEST_BUILD") != "1" and guest_path == default_guest_path:
            build_guest_module(guest_manifest)
            result["guest_manifest"] = str(guest_manifest)
            result["guest_build_mode"] = "cargo+stable"

        browser = start_remote_daemon(name=name, timeout=timeout)
        result["browser_id"] = browser["id"]

        result["drain_before_guest"], result["drain_before_guest_request"] = run_runner_ndjson_command(
            "watch-events",
            {
                "daemon_name": name,
                "filter": {},
                "timeout_ms": 250,
                "poll_interval_ms": 50,
            },
            timeout_seconds=10,
        )

        serve_guest = start_serve_guest_process(guest_path)
        guest_config = {
            "daemon_name": name,
            "guest_module": str(guest_path),
            "granted_operations": [
                "goto",
                "wait_for_load_event",
                "js",
                "page_info",
            ],
            "allow_http": False,
            "allow_raw_cdp": False,
            "persistent_guest_state": True,
        }
        result["ready"] = send_line(
            serve_guest,
            {
                "command": "start",
                "config": guest_config,
            },
        )
        result["status_after_start"] = send_line(serve_guest, {"command": "status"})
        result["first_run"] = send_line(serve_guest, {"command": "run"})
        result["second_run"] = send_line(serve_guest, {"command": "run"})
        result["status_after_runs"] = send_line(serve_guest, {"command": "status"})
        result["stopped"] = send_line(serve_guest, {"command": "stop"})
        stderr = finish_serve_guest(serve_guest)
        if stderr:
            result["serve_guest_stderr"] = stderr
        serve_guest = None

        ready = result["ready"]
        if ready.get("kind") != "ready":
            raise RuntimeError(f"unexpected ready response: {ready!r}")
        if ready.get("invocation_count") != 0:
            raise RuntimeError(f"unexpected ready invocation count: {ready!r}")

        status_after_start = result["status_after_start"]
        if status_after_start.get("kind") != "status":
            raise RuntimeError(f"unexpected status response after start: {status_after_start!r}")
        if status_after_start.get("invocation_count") != 0:
            raise RuntimeError(f"unexpected initial status invocation count: {status_after_start!r}")

        first_run = result["first_run"]
        second_run = result["second_run"]
        if first_run.get("kind") != "run_result":
            raise RuntimeError(f"unexpected first run response: {first_run!r}")
        if second_run.get("kind") != "run_result":
            raise RuntimeError(f"unexpected second run response: {second_run!r}")
        if first_run.get("invocation_count") != 1:
            raise RuntimeError(f"unexpected first invocation count: {first_run!r}")
        if second_run.get("invocation_count") != 2:
            raise RuntimeError(f"unexpected second invocation count: {second_run!r}")

        first_result = first_run.get("result") or {}
        second_result = second_run.get("result") or {}
        if not first_result.get("success"):
            raise RuntimeError(f"first guest run failed: {first_result!r}")
        if not second_result.get("success"):
            raise RuntimeError(f"second guest run failed: {second_result!r}")

        first_ops = [call.get("operation") for call in first_result.get("calls") or []]
        second_ops = [call.get("operation") for call in second_result.get("calls") or []]
        result["first_run_operations"] = first_ops
        result["second_run_operations"] = second_ops
        if first_ops != ["goto", "wait_for_load_event", "js", "page_info"]:
            raise RuntimeError(f"unexpected first guest operation sequence: {first_ops!r}")
        if second_ops != ["js", "page_info"]:
            raise RuntimeError(f"unexpected second guest operation sequence: {second_ops!r}")

        first_js_response = (first_result.get("calls") or [])[2].get("response")
        if first_js_response != "phase-1":
            raise RuntimeError(f"unexpected first guest js response: {first_js_response!r}")

        first_page_info = (first_result.get("calls") or [])[3].get("response") or {}
        if first_page_info.get("url") != target_url:
            raise RuntimeError("first guest page_info did not report the target URL")

        second_js_response = (second_result.get("calls") or [])[0].get("response")
        try:
            second_js_state = json.loads(second_js_response)
        except json.JSONDecodeError as exc:
            raise RuntimeError(f"second guest js response was not JSON: {second_js_response!r}") from exc
        result["second_js_state"] = second_js_state
        if second_js_state.get("href") != target_url:
            raise RuntimeError("second guest js href did not match the target URL")
        if second_js_state.get("marker") != "phase-1":
            raise RuntimeError("second guest js marker did not preserve browser state")

        second_page_info = (second_result.get("calls") or [])[1].get("response") or {}
        if second_page_info.get("url") != target_url:
            raise RuntimeError("second guest page_info did not report the target URL")

        status_after_runs = result["status_after_runs"]
        if status_after_runs.get("kind") != "status":
            raise RuntimeError(f"unexpected status response after runs: {status_after_runs!r}")
        if status_after_runs.get("invocation_count") != 2:
            raise RuntimeError(f"unexpected post-run status invocation count: {status_after_runs!r}")

        stopped = result["stopped"]
        if stopped.get("kind") != "stopped":
            raise RuntimeError(f"unexpected stop response: {stopped!r}")
        if stopped.get("invocation_count") != 2:
            raise RuntimeError(f"unexpected stop invocation count: {stopped!r}")

        result["page_after_guest"], result["page_after_guest_request"] = run_runner_command(
            "page-info",
            {"daemon_name": name},
        )
        if result["page_after_guest"].get("url") != target_url:
            raise RuntimeError("runner page-info after guest did not match the target URL")

        result["marker_after_guest"], result["marker_after_guest_request"] = run_runner_command(
            "js",
            {"daemon_name": name, "expression": "window.__bhrunPersistentMarker"},
        )
        if result["marker_after_guest"] != "phase-1":
            raise RuntimeError("runner js after guest did not preserve the marker")
    finally:
        if serve_guest is not None:
            try:
                finish_serve_guest(serve_guest)
            except Exception as exc:
                result["serve_guest_cleanup_error"] = str(exc)
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
