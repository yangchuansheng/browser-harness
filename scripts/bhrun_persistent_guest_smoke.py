#!/usr/bin/env python3
"""Run a local end-to-end smoke test for `bhrun serve-guest`.

Optional:
  BU_RUST_RUNNER_BIN  override the bhrun binary path
"""

import json
import os
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
GUEST = REPO / "rust" / "guests" / "persistent_counter.wat"


def runner_process_spec():
    if custom := os.environ.get("BU_RUST_RUNNER_BIN"):
        return [custom], str(REPO)
    return ["cargo", "run", "--quiet", "--bin", "bhrun", "--"], str(REPO / "rust")


def send_line(proc, payload):
    if proc.stdin is None or proc.stdout is None:
        raise RuntimeError("runner process pipes are not available")
    proc.stdin.write(json.dumps(payload) + "\n")
    proc.stdin.flush()
    line = proc.stdout.readline()
    if not line:
        stderr = ""
        if proc.stderr is not None:
            stderr = proc.stderr.read().strip()
        raise RuntimeError(stderr or "serve-guest returned no output")
    return json.loads(line)


def main():
    cmd, cwd = runner_process_spec()
    proc = subprocess.Popen(
        cmd + ["serve-guest", str(GUEST)],
        cwd=cwd,
        env=os.environ.copy(),
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    result = {
        "guest_path": str(GUEST),
        "runner_command": cmd + ["serve-guest", str(GUEST)],
    }

    try:
        config = {
            "daemon_name": "default",
            "guest_module": str(GUEST),
            "granted_operations": ["wait"],
            "allow_http": False,
            "allow_raw_cdp": False,
            "persistent_guest_state": True,
        }
        result["ready"] = send_line(
            proc,
            {
                "command": "start",
                "config": config,
            },
        )
        result["status"] = send_line(proc, {"command": "status"})
        result["first_run"] = send_line(proc, {"command": "run"})
        result["second_run"] = send_line(proc, {"command": "run"})
        result["stopped"] = send_line(proc, {"command": "stop"})
    finally:
        if proc.stdin is not None:
            proc.stdin.close()
        stderr = ""
        if proc.stderr is not None:
            stderr = proc.stderr.read().strip()
        proc.wait(timeout=10)
        if stderr:
            result["stderr"] = stderr

    first_duration = (
        result["first_run"]
        .get("result", {})
        .get("calls", [{}])[0]
        .get("request", {})
        .get("duration_ms")
    )
    second_duration = (
        result["second_run"]
        .get("result", {})
        .get("calls", [{}])[0]
        .get("request", {})
        .get("duration_ms")
    )

    if result["ready"].get("kind") != "ready":
        raise RuntimeError(f"unexpected ready response: {result['ready']!r}")
    if result["status"].get("kind") != "status":
        raise RuntimeError(f"unexpected status response: {result['status']!r}")
    if result["first_run"].get("invocation_count") != 1:
        raise RuntimeError(f"unexpected first invocation count: {result['first_run']!r}")
    if result["second_run"].get("invocation_count") != 2:
        raise RuntimeError(f"unexpected second invocation count: {result['second_run']!r}")
    if first_duration != 1:
        raise RuntimeError(f"unexpected first guest duration: {first_duration!r}")
    if second_duration != 2:
        raise RuntimeError(f"unexpected second guest duration: {second_duration!r}")
    if result["stopped"].get("invocation_count") != 2:
        raise RuntimeError(f"unexpected stop response: {result['stopped']!r}")

    print(json.dumps(result))


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:  # pragma: no cover - simple smoke script
        raise SystemExit(str(exc))
