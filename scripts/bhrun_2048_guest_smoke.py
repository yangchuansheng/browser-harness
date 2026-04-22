#!/usr/bin/env python3
"""Run a smoke for the Rust/Wasm 2048 autoplay guest.

Optional:
  BU_NAME                      defaults to "bhrun-2048-guest-smoke"
  BU_BROWSER_MODE              defaults to "local"; set to "remote" to use Browser Use
  BROWSER_USE_API_KEY          required in remote mode
  BU_DAEMON_IMPL               defaults to "rust"
  BU_REMOTE_TIMEOUT_MINUTES    defaults to "1"
  BU_LOCAL_DAEMON_WAIT_SECONDS defaults to "15"
  BU_2048_TARGET               defaults to "512"
  BU_GUEST_PATH                override the guest module path
  BU_SKIP_GUEST_BUILD          set to "1" to skip the default Rust guest build
  BU_RUST_RUNNER_BIN           override the bhrun binary path
"""

import json
import os
import subprocess
import sys
import time
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO))
os.environ.setdefault("BU_NAME", "bhrun-2048-guest-smoke")

from scripts._admin_cli import (  # noqa: E402
    ensure_daemon,
    list_browsers,
    restart_daemon,
    start_remote_daemon,
)

TARGET_URL = "https://play2048.co/?via=bhrun-2048-guest-smoke"
SCORE_SCRIPT = r"""
JSON.stringify((() => {
  const lines = document.body.innerText.split(/\n+/).map(s => s.trim()).filter(Boolean);
  const scoreIndex = lines.indexOf("SCORE");
  const bestIndex = lines.indexOf("BEST");
  const score = scoreIndex >= 0 && scoreIndex + 1 < lines.length ? Number(lines[scoreIndex + 1]) || 0 : 0;
  const best = bestIndex >= 0 && bestIndex + 1 < lines.length ? Number(lines[bestIndex + 1]) || 0 : 0;
  return {
    score,
    best,
    adTextPresent: /A Message from Samsung|LEARN MORE|Get the App/i.test(document.body.innerText),
    bodyHead: document.body ? document.body.innerText.slice(0, 500) : null,
  };
})())
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
            "failed to build the Rust 2048 autoplay guest; ensure the stable wasm target is installed "
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
    browser_mode = os.environ.get("BU_BROWSER_MODE", "local").strip().lower() or "local"
    if browser_mode not in {"local", "remote"}:
        raise SystemExit("BU_BROWSER_MODE must be 'local' or 'remote'")
    if browser_mode == "remote" and not os.environ.get("BROWSER_USE_API_KEY"):
        raise SystemExit("BROWSER_USE_API_KEY is required in remote mode")

    os.environ.setdefault("BU_DAEMON_IMPL", "rust")
    name = os.environ["BU_NAME"]
    timeout = int(os.environ.get("BU_REMOTE_TIMEOUT_MINUTES", "1"))
    local_wait = float(os.environ.get("BU_LOCAL_DAEMON_WAIT_SECONDS", "15"))
    target_score = int(os.environ.get("BU_2048_TARGET", "512"))
    guest_manifest = REPO / "rust" / "guests" / "rust-2048-autoplay" / "Cargo.toml"
    default_guest_path = (
        REPO
        / "rust"
        / "guests"
        / "rust-2048-autoplay"
        / "target"
        / "wasm32-unknown-unknown"
        / "release"
        / "rust_2048_autoplay_guest.wasm"
    )
    guest_path = Path(os.environ.get("BU_GUEST_PATH", str(default_guest_path)))

    browser = None
    result = {
        "name": name,
        "daemon_impl": os.environ["BU_DAEMON_IMPL"],
        "browser_mode": browser_mode,
        "target_score": target_score,
        "guest_path": str(guest_path),
        "target_url": TARGET_URL,
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

        run_runner_command("goto", {"daemon_name": name, "url": TARGET_URL}, timeout_seconds=20)
        run_runner_command("wait-for-load", {"daemon_name": name, "timeout": 15.0}, timeout_seconds=20)
        run_runner_command("wait", {"daemon_name": name, "duration_ms": 3000}, timeout_seconds=10)
        run_runner_command(
            "js",
            {
                "daemon_name": name,
                "expression": f"localStorage.setItem('bh2048GuestTarget', {json.dumps(str(target_score))}); 'ok'",
            },
            timeout_seconds=10,
        )

        sample_config, _ = run_runner_command("sample-config")
        sample_config["daemon_name"] = name
        sample_config["guest_module"] = str(guest_path)
        sample_config["allow_raw_cdp"] = True
        sample_config["granted_operations"] = [
            "cdp_raw",
            "goto",
            "wait_for_load",
            "wait",
            "page_info",
            "js",
        ]
        result["guest_config"] = sample_config

        guest_run, _ = run_runner_command(
            "run-guest",
            sample_config,
            timeout_seconds=120,
            extra_args=[str(guest_path)],
        )
        result["guest_run"] = guest_run
        calls = guest_run.get("calls") or []
        operations = [call.get("operation") for call in calls]
        result["guest_operations"] = operations

        if not guest_run.get("success"):
            raise RuntimeError(f"guest run failed: {json.dumps(result, sort_keys=True)}")
        if guest_run.get("exit_code") != 0:
            raise RuntimeError(f"unexpected guest exit code: {json.dumps(result, sort_keys=True)}")
        if operations[:2] != ["cdp_raw", "goto"]:
            raise RuntimeError(f"unexpected guest start sequence: {operations!r}")
        if "js" not in operations:
            raise RuntimeError(f"guest never called js: {operations!r}")

        raw_score_payload, score_request = run_runner_command(
            "js",
            {"daemon_name": name, "expression": SCORE_SCRIPT},
            timeout_seconds=10,
        )
        score_payload = json.loads(raw_score_payload)
        result["page_score"] = score_payload
        result["page_score_request"] = score_request

        if int(score_payload.get("score", 0)) < target_score:
            raise RuntimeError(
                f"guest did not reach the requested score: {score_payload.get('score')} < {target_score}"
            )
        if score_payload.get("adTextPresent"):
            raise RuntimeError(f"guest left obvious ad text on the page: {json.dumps(score_payload)}")
    finally:
        restart_daemon(name)
        time.sleep(1)
        if browser is not None:
            result["post_shutdown_status"] = poll_browser_status(browser["id"])
        log_path = Path(f"/tmp/bu-{name}.log")
        if log_path.exists():
            result["log_tail"] = log_path.read_text().strip().splitlines()[-12:]

    print(json.dumps(result))


if __name__ == "__main__":
    main()
