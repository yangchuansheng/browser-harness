#!/usr/bin/env python3
"""Run a smoke for the Rust/Wasm event-waits SDK guest.

Optional:
  BU_NAME                      defaults to "bhrun-event-waits-guest-smoke"
  BU_BROWSER_MODE              defaults to "local"; set to "remote" for Browser Use
  BU_DAEMON_IMPL               defaults to "rust"
  BU_REMOTE_TIMEOUT_MINUTES    defaults to "1"
  BU_LOCAL_DAEMON_WAIT_SECONDS defaults to "15"
  BU_GUEST_PATH                override the guest module path
  BU_SKIP_GUEST_BUILD          set to "1" to skip the default Rust guest build
  BU_RUST_RUNNER_BIN           override the bhrun binary path

Required in remote mode:
  BROWSER_USE_API_KEY
"""

import json
import os
import subprocess
import sys
import time
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO))
os.environ.setdefault("BU_NAME", "bhrun-event-waits-guest-smoke")

from scripts._admin_cli import (  # noqa: E402
    ensure_daemon,
    list_browsers,
    restart_daemon,
    start_remote_daemon,
)

WAIT_EVENT_TOKEN = "bhrun-event-wait"
WATCH_TOKEN_ONE = "bhrun-event-watch-1"
WATCH_TOKEN_TWO = "bhrun-event-watch-2"
CONSOLE_TOKEN = "bhrun-event-console"
DIALOG_TOKEN = "bhrun-event-dialog"


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


def daemon_process_spec():
    if custom := os.environ.get("BU_RUST_DAEMON_BIN"):
        return [custom], str(REPO)
    binary = REPO / "rust" / "target" / "debug" / "bhd"
    proc = subprocess.run(
        ["cargo", "build", "--quiet", "--bin", "bhd"],
        cwd=REPO / "rust",
        env=os.environ.copy(),
        text=True,
        capture_output=True,
    )
    if proc.returncode != 0:
        detail = (proc.stderr or proc.stdout or "daemon build failed").strip()
        raise RuntimeError(f"failed to build bhd for local attach\n{detail}")
    return [str(binary)], str(REPO)


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
            "failed to build the Rust event-waits guest; ensure the stable wasm target is installed "
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


def cleanup_dialog_best_effort(name):
    try:
        result, request = run_runner_command(
            "handle-dialog",
            {"daemon_name": name, "action": "accept"},
            timeout_seconds=5,
        )
    except Exception:
        return None
    time.sleep(0.2)
    try:
        page, page_request = run_runner_command(
            "page-info",
            {"daemon_name": name},
            timeout_seconds=5,
        )
    except Exception:
        page = None
        page_request = None
    return {
        "handle_dialog_result": result,
        "handle_dialog_request": request,
        "page_info": page,
        "page_info_request": page_request,
    }


def wait_for_daemon_ready(name, timeout_seconds):
    deadline = time.time() + timeout_seconds
    last_error = None
    while time.time() < deadline:
        try:
            session, _ = run_runner_command(
                "current-session",
                {"daemon_name": name},
                timeout_seconds=5,
            )
            return session
        except Exception as err:
            last_error = err
            time.sleep(0.2)
    raise RuntimeError(f"local daemon did not become ready: {last_error}")


def ensure_local_daemon(name, wait_seconds):
    if os.environ.get("BU_CDP_WS"):
        try:
            wait_for_daemon_ready(name, timeout_seconds=1.0)
            return "BU_CDP_WS(existing)"
        except Exception:
            restart_daemon(name)
            cmd, cwd = daemon_process_spec()
            env = os.environ.copy()
            env["BU_NAME"] = name
            subprocess.Popen(
                cmd,
                cwd=cwd,
                env=env,
                stdin=subprocess.DEVNULL,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
            )
            wait_for_daemon_ready(name, timeout_seconds=wait_seconds)
            return "BU_CDP_WS"

    ensure_daemon(name=name, wait=wait_seconds)
    return "DevToolsActivePort"


def main():
    browser_mode = os.environ.get("BU_BROWSER_MODE", "local").strip().lower() or "local"
    if browser_mode not in {"remote", "local"}:
        raise SystemExit("BU_BROWSER_MODE must be 'remote' or 'local'")
    if browser_mode == "remote" and not os.environ.get("BROWSER_USE_API_KEY"):
        raise SystemExit("BROWSER_USE_API_KEY is required")

    os.environ.setdefault("BU_DAEMON_IMPL", "rust")
    name = os.environ["BU_NAME"]
    timeout = int(os.environ.get("BU_REMOTE_TIMEOUT_MINUTES", "1"))
    local_wait = float(os.environ.get("BU_LOCAL_DAEMON_WAIT_SECONDS", "15"))
    guest_manifest = REPO / "rust" / "guests" / "rust-event-waits-sdk" / "Cargo.toml"
    default_guest_path = (
        REPO
        / "rust"
        / "guests"
        / "rust-event-waits-sdk"
        / "target"
        / "wasm32-unknown-unknown"
        / "release"
        / "rust_event_waits_sdk_guest.wasm"
    )
    guest_path = Path(os.environ.get("BU_GUEST_PATH", str(default_guest_path)))

    browser = None
    result = {
        "name": name,
        "daemon_impl": os.environ["BU_DAEMON_IMPL"],
        "browser_mode": browser_mode,
        "guest_path": str(guest_path),
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
            result["local_attach"] = ensure_local_daemon(name=name, wait_seconds=local_wait)

        result["prewarm_tab"], result["prewarm_tab_request"] = run_runner_command(
            "new-tab",
            {"daemon_name": name, "url": "about:blank"},
            timeout_seconds=10,
        )

        sample_config, _ = run_runner_command("sample-config")
        sample_config["daemon_name"] = name
        sample_config["guest_module"] = str(guest_path)
        sample_config["granted_operations"] = [
            "current_session",
            "wait_for_event",
            "watch_events",
            "wait_for_console",
            "wait_for_dialog",
            "handle_dialog",
            "js",
        ]
        result["guest_config"] = sample_config

        guest_run, _ = run_runner_command(
            "run-guest",
            sample_config,
            timeout_seconds=25,
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
            "current_session",
            "js",
            "wait_for_event",
            "js",
            "watch_events",
            "js",
            "wait_for_console",
            "js",
            "wait_for_dialog",
            "handle_dialog",
        ]
        if operations != expected_operations:
            raise RuntimeError(f"unexpected guest operation sequence: {operations!r}")

        session_id = (calls[0].get("response") or {}).get("session_id")
        if not session_id:
            raise RuntimeError("guest current_session response did not include session_id")

        wait_event = calls[2].get("response") or {}
        if not wait_event.get("matched"):
            raise RuntimeError("guest wait_for_event returned matched=false")
        if (wait_event.get("event") or {}).get("method") != "Runtime.consoleAPICalled":
            raise RuntimeError("guest wait_for_event did not return Runtime.consoleAPICalled")
        if (wait_event.get("event") or {}).get("session_id") != session_id:
            raise RuntimeError("guest wait_for_event session mismatch")
        wait_token = (
            (wait_event.get("event") or {})
            .get("params", {})
            .get("args", [{}])[0]
            .get("value")
        )
        if wait_token != WAIT_EVENT_TOKEN:
            raise RuntimeError(f"guest wait_for_event token mismatch: {wait_token!r}")

        watched = calls[4].get("response") or []
        result["watched_lines"] = watched
        if len(watched) != 3:
            raise RuntimeError(f"guest watch_events returned unexpected line count: {len(watched)}")
        if watched[0].get("kind") != "event" or watched[1].get("kind") != "event":
            raise RuntimeError("guest watch_events did not return event lines first")
        watch_one = (
            watched[0]
            .get("event", {})
            .get("params", {})
            .get("args", [{}])[0]
            .get("value")
        )
        watch_two = (
            watched[1]
            .get("event", {})
            .get("params", {})
            .get("args", [{}])[0]
            .get("value")
        )
        if [watch_one, watch_two] != [WATCH_TOKEN_ONE, WATCH_TOKEN_TWO]:
            raise RuntimeError(f"guest watch_events tokens did not match: {[watch_one, watch_two]!r}")
        end_line = watched[2]
        if end_line.get("kind") != "end":
            raise RuntimeError("guest watch_events did not end with an end line")
        if end_line.get("matched_events") != 2 or not end_line.get("reached_max_events"):
            raise RuntimeError(f"guest watch_events end summary was unexpected: {end_line!r}")

        console_wait = calls[6].get("response") or {}
        console_event = console_wait.get("event") or {}
        if not console_wait.get("matched"):
            raise RuntimeError("guest wait_for_console returned matched=false")
        if console_event.get("session_id") != session_id:
            raise RuntimeError("guest wait_for_console session mismatch")
        if console_event.get("method") == "Console.messageAdded":
            console_text = console_event.get("params", {}).get("message", {}).get("text")
        else:
            console_text = (
                console_event.get("params", {}).get("args", [{}])[0].get("value")
                or console_event.get("params", {}).get("args", [{}])[0].get("description")
            )
        if console_text != CONSOLE_TOKEN:
            raise RuntimeError(f"guest wait_for_console token mismatch: {console_text!r}")

        dialog_wait = calls[8].get("response") or {}
        dialog_event = dialog_wait.get("event") or {}
        if not dialog_wait.get("matched"):
            raise RuntimeError("guest wait_for_dialog returned matched=false")
        if dialog_event.get("method") != "Page.javascriptDialogOpening":
            raise RuntimeError("guest wait_for_dialog did not return dialog opening")
        if dialog_event.get("session_id") != session_id:
            raise RuntimeError("guest wait_for_dialog session mismatch")
        if dialog_event.get("params", {}).get("message") != DIALOG_TOKEN:
            raise RuntimeError("guest wait_for_dialog message mismatch")

        result["handle_dialog_response"] = calls[9].get("response")
        result["post_run_page_info"], result["post_run_page_info_request"] = run_runner_command(
            "page-info",
            {"daemon_name": name},
            timeout_seconds=5,
        )
        if "dialog" in result["post_run_page_info"]:
            raise RuntimeError("dialog was still pending after guest handle_dialog")
    finally:
        try:
            if "post_run_page_info" not in result:
                result["page_after_cleanup_attempt"] = cleanup_dialog_best_effort(name)
        except Exception as err:
            result["cleanup_error"] = str(err)
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
