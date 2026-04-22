#!/usr/bin/env python3
"""Run a live end-to-end smoke test for `bhrun screenshot`.

Optional:
  BU_NAME                   defaults to "bhrun-screenshot-smoke"
  BU_BROWSER_MODE           defaults to "remote"; set to "local" to attach via DevToolsActivePort
  BU_DAEMON_IMPL            defaults to "rust"
  BU_REMOTE_TIMEOUT_MINUTES defaults to "1"
  BU_LOCAL_DAEMON_WAIT_SECONDS defaults to "15"
  BU_RUST_RUNNER_BIN        override the bhrun binary path
"""

import base64
import json
import os
import struct
import subprocess
import sys
import time
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO))
os.environ.setdefault("BU_NAME", "bhrun-screenshot-smoke")

from admin import _browser_use, ensure_daemon, restart_daemon, start_remote_daemon  # noqa: E402
from scripts._runner_cli import js, new_tab, page_info, wait_for_load  # noqa: E402

TARGET_URL = "https://example.com/?via=bhrun-screenshot-smoke"


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


def run_runner_command(subcommand, payload=None, timeout_seconds=15):
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


def decode_png_dimensions(encoded_png):
    png = base64.b64decode(encoded_png)
    if png[:8] != b"\x89PNG\r\n\x1a\n":
        raise RuntimeError("runner screenshot did not return a PNG")
    width, height = struct.unpack(">II", png[16:24])
    return png, width, height


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
        "target_url": TARGET_URL,
    }
    try:
        if browser_mode == "remote":
            browser = start_remote_daemon(name=name, timeout=timeout)
            result["browser_id"] = browser["id"]
        else:
            ensure_daemon(name=name, wait=local_wait)
            result["local_attach"] = "DevToolsActivePort"

        result["target_id"] = new_tab(TARGET_URL)
        result["loaded"] = wait_for_load()
        result["page_before_setup"] = page_info()

        tall_layout = js(
            """
(() => {
  const marker = document.createElement('div');
  marker.id = 'bhrun-screenshot-smoke-marker';
  marker.textContent = 'full-shot-marker';
  marker.style.cssText = [
    'display:block',
    'height:3200px',
    'background:linear-gradient(#ffffff,#d6e4ff)',
    'border-top:8px solid #345'
  ].join(';');
  document.body.style.margin = '0';
  document.body.appendChild(marker);
  window.scrollTo(0, 0);
  return {
    marker: marker.textContent,
    scrollHeight: document.documentElement.scrollHeight
  };
})()
"""
        )
        result["layout_setup"] = tall_layout
        result["page_after_setup"] = page_info()
        if result["page_after_setup"].get("ph", 0) <= result["page_after_setup"].get("h", 0):
            raise RuntimeError("page did not become taller than the viewport before full screenshot")

        viewport_png_b64, viewport_request = run_runner_command(
            "screenshot",
            {"daemon_name": name, "full": False},
            timeout_seconds=20,
        )
        full_png_b64, full_request = run_runner_command(
            "screenshot",
            {"daemon_name": name, "full": True},
            timeout_seconds=20,
        )
        result["viewport_request"] = viewport_request
        result["full_request"] = full_request

        viewport_png, viewport_width, viewport_height = decode_png_dimensions(viewport_png_b64)
        full_png, full_width, full_height = decode_png_dimensions(full_png_b64)
        result["viewport_png_bytes"] = len(viewport_png)
        result["full_png_bytes"] = len(full_png)
        result["viewport_png_dimensions"] = {"width": viewport_width, "height": viewport_height}
        result["full_png_dimensions"] = {"width": full_width, "height": full_height}

        if viewport_width <= 0 or viewport_height <= 0:
            raise RuntimeError("viewport screenshot dimensions were invalid")
        if full_width <= 0 or full_height <= 0:
            raise RuntimeError("full screenshot dimensions were invalid")
        if full_height <= viewport_height:
            raise RuntimeError(
                f"full screenshot height did not exceed viewport height: {full_height} <= {viewport_height}"
            )
        if full_width + 128 < viewport_width:
            raise RuntimeError(
                "full screenshot width shrank more than a scrollbar-sized tolerance: "
                f"{full_width} << {viewport_width}"
            )

        result["page_after_screenshots"] = page_info()
        if result["page_after_screenshots"].get("url") != TARGET_URL:
            raise RuntimeError("page URL changed during screenshot capture")
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
