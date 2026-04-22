#!/usr/bin/env python3
"""Run a live end-to-end smoke test for `bhrun set-viewport`.

Optional:
  BU_NAME                   defaults to "bhrun-viewport-smoke"
  BU_BROWSER_MODE           defaults to "local"; "remote" is best-effort only
  BU_DAEMON_IMPL            defaults to "rust"
  BU_REMOTE_TIMEOUT_MINUTES defaults to "1"
  BU_LOCAL_DAEMON_WAIT_SECONDS defaults to "15"
"""

import json
import os
import sys
import time
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO))
os.environ.setdefault("BU_NAME", "bhrun-viewport-smoke")

from scripts._admin_cli import (  # noqa: E402
    ensure_daemon,
    list_browsers,
    restart_daemon,
    start_remote_daemon,
)
from scripts._runner_cli import goto, js, page_info, set_viewport, wait, wait_for_load  # noqa: E402

TARGET_URL = "https://example.com/?via=bhrun-viewport-smoke"


def poll_browser_status(browser_id, attempts=10, delay=1.0):
    for _ in range(attempts):
        listing = list_browsers(page_size=20, page_number=1)
        item = next((item for item in listing.get("items", []) if item.get("id") == browser_id), None)
        status = item.get("status") if item else "missing"
        if status != "active":
            return status
        time.sleep(delay)
    return status


def assert_page_size(page, width, height, label):
    if page.get("w") != width or page.get("h") != height:
        raise RuntimeError(
            f"{label} viewport mismatch: expected {width}x{height}, got {page.get('w')}x{page.get('h')}"
        )


def assert_dpr(actual, expected, label):
    if not isinstance(actual, (int, float)) or abs(float(actual) - expected) > 0.05:
        raise RuntimeError(f"{label} viewport expected devicePixelRatio {expected}, got {actual}")


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

        goto(TARGET_URL, daemon_name=name)
        result["loaded"] = wait_for_load(daemon_name=name)
        initial_page = page_info(daemon_name=name)
        result["initial_page"] = initial_page
        initial_width = initial_page.get("w")
        initial_height = initial_page.get("h")
        if not isinstance(initial_width, int) or not isinstance(initial_height, int):
            raise RuntimeError(f"initial page_info did not expose integer viewport size: {initial_page}")

        desktop_request = {
            "width": 900,
            "height": 700,
            "device_scale_factor": 1.0,
            "mobile": False,
        }
        set_viewport(daemon_name=name, timeout_seconds=10, **desktop_request)
        wait(0.3)
        desktop_page = page_info(daemon_name=name)
        desktop_metrics = js(
            "({width: innerWidth, height: innerHeight, dpr: window.devicePixelRatio})",
            daemon_name=name,
        )
        result["desktop_request"] = desktop_request
        result["desktop_page"] = desktop_page
        result["desktop_metrics"] = desktop_metrics
        assert_page_size(desktop_page, 900, 700, "desktop")
        assert_dpr(desktop_metrics.get("dpr"), 1.0, "desktop")

        mobile_request = {
            "width": 480,
            "height": 720,
            "device_scale_factor": 2.0,
            "mobile": True,
        }
        set_viewport(daemon_name=name, timeout_seconds=10, **mobile_request)
        wait(0.3)
        mobile_page = page_info(daemon_name=name)
        mobile_metrics = js(
            """
(() => ({
  width: innerWidth,
  height: innerHeight,
  dpr: window.devicePixelRatio,
  coarse: matchMedia('(pointer: coarse)').matches,
  reducedHover: matchMedia('(hover: none)').matches
}))()
""",
            daemon_name=name,
        )
        result["mobile_request"] = mobile_request
        result["mobile_page"] = mobile_page
        result["mobile_metrics"] = mobile_metrics
        assert_page_size(mobile_page, 480, 720, "mobile")
        assert_dpr(mobile_metrics.get("dpr"), 2.0, "mobile")

        set_viewport(
            initial_width,
            initial_height,
            device_scale_factor=1.0,
            mobile=False,
            daemon_name=name,
            timeout_seconds=10,
        )
        wait(0.3)
        restored_page = page_info(daemon_name=name)
        result["restored_page"] = restored_page
        if restored_page.get("w") != initial_width or restored_page.get("h") != initial_height:
            raise RuntimeError(
                "viewport did not restore close to the initial size: "
                f"{restored_page.get('w')}x{restored_page.get('h')} vs {initial_width}x{initial_height}"
            )
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
