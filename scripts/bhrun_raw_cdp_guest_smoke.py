#!/usr/bin/env python3
"""Run a smoke for the Rust-native raw CDP escape hatch.

Optional:
  BU_NAME                      defaults to "bhrun-raw-cdp-guest-smoke"
  BU_BROWSER_MODE              defaults to "local"; set to "remote" for Browser Use
  BU_DAEMON_IMPL               defaults to "rust"
  BU_REMOTE_TIMEOUT_MINUTES    defaults to "1"
  BU_LOCAL_DAEMON_WAIT_SECONDS defaults to "15"
  BU_GUEST_PATH                override the guest module path
  BU_SKIP_GUEST_BUILD          set to "1" to skip the default Rust guest build
  BU_RUST_RUNNER_BIN           override the bhrun binary path
  BU_RUST_DAEMON_BIN           override the bhd binary path

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
os.environ.setdefault("BU_NAME", "bhrun-raw-cdp-guest-smoke")

from admin import _browser_use, ensure_daemon, restart_daemon, start_remote_daemon  # noqa: E402

CLI_TOKEN = "bhrun-raw-cdp-cli"
GUEST_TOKEN = "bhrun-raw-cdp-guest"


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


def daemon_process_spec():
    if custom := os.environ.get("BU_RUST_DAEMON_BIN"):
        return [custom], str(REPO)
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
    return [str(REPO / "rust" / "target" / "debug" / "bhd")], str(REPO)


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
            "failed to build the Rust raw CDP guest; ensure the stable wasm target is installed "
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
    guest_manifest = REPO / "rust" / "guests" / "rust-raw-cdp-smoke" / "Cargo.toml"
    default_guest_path = (
        REPO
        / "rust"
        / "guests"
        / "rust-raw-cdp-smoke"
        / "target"
        / "wasm32-unknown-unknown"
        / "release"
        / "rust_raw_cdp_smoke_guest.wasm"
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

        current_session, current_session_request = run_runner_command(
            "current-session",
            {"daemon_name": name},
            timeout_seconds=10,
        )
        result["current_session"] = current_session
        result["current_session_request"] = current_session_request
        session_id = current_session.get("session_id")
        if not session_id:
            raise RuntimeError("current-session did not return a session_id")

        cli_raw_result, cli_raw_request = run_runner_command(
            "cdp-raw",
            {
                "daemon_name": name,
                "method": "Runtime.evaluate",
                "session_id": session_id,
                "params": {
                    "expression": f"window.__bhrunRawCdpCli = {json.dumps(CLI_TOKEN)}; window.__bhrunRawCdpCli",
                    "returnByValue": True,
                    "awaitPromise": True,
                },
            },
            timeout_seconds=10,
        )
        result["cli_raw_result"] = cli_raw_result
        result["cli_raw_request"] = cli_raw_request
        if cli_raw_result.get("result", {}).get("value") != CLI_TOKEN:
            raise RuntimeError(f"unexpected cli cdp-raw result: {cli_raw_result!r}")

        sample_config, _ = run_runner_command("sample-config")
        sample_config["daemon_name"] = name
        sample_config["guest_module"] = str(guest_path)
        sample_config["granted_operations"] = ["current_session", "cdp_raw"]

        disabled_config = dict(sample_config)
        disabled_config["allow_raw_cdp"] = False
        disabled_run, _ = run_runner_command(
            "run-guest",
            disabled_config,
            timeout_seconds=20,
            extra_args=[str(guest_path)],
        )
        result["guest_run_disabled"] = disabled_run
        if disabled_run.get("success"):
            raise RuntimeError("guest unexpectedly succeeded with allow_raw_cdp=false")
        if "cdp_raw disabled" not in str(disabled_run.get("trap")):
            raise RuntimeError(f"guest did not report the raw CDP gate: {disabled_run!r}")

        enabled_config = dict(sample_config)
        enabled_config["allow_raw_cdp"] = True
        enabled_run, _ = run_runner_command(
            "run-guest",
            enabled_config,
            timeout_seconds=20,
            extra_args=[str(guest_path)],
        )
        result["guest_run_enabled"] = enabled_run
        if not enabled_run.get("success"):
            raise RuntimeError(f"guest run failed with allow_raw_cdp=true: {enabled_run!r}")
        if enabled_run.get("exit_code") != 0:
            raise RuntimeError(f"unexpected guest exit code: {enabled_run.get('exit_code')!r}")

        calls = enabled_run.get("calls") or []
        operations = [call.get("operation") for call in calls]
        result["guest_operations"] = operations
        if operations != ["current_session", "cdp_raw"]:
            raise RuntimeError(f"unexpected guest operation sequence: {operations!r}")
        guest_response = calls[1].get("response") or {}
        if guest_response.get("result", {}).get("value") != GUEST_TOKEN:
            raise RuntimeError(f"unexpected guest raw CDP response: {guest_response!r}")
        if calls[1].get("request", {}).get("session_id") != calls[0].get("response", {}).get("session_id"):
            raise RuntimeError("guest cdp_raw did not use the active session_id")
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
