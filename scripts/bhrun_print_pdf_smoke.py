#!/usr/bin/env python3
"""Run a live end-to-end smoke test for `bhrun print-pdf`.

Optional:
  BU_NAME                   defaults to "bhrun-print-pdf-smoke"
  BU_BROWSER_MODE           defaults to "local"; "remote" is best-effort only
  BU_DAEMON_IMPL            defaults to "rust"
  BU_REMOTE_TIMEOUT_MINUTES defaults to "1"
  BU_LOCAL_DAEMON_WAIT_SECONDS defaults to "15"
"""

import base64
import json
import os
import sys
import time
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO))
os.environ.setdefault("BU_NAME", "bhrun-print-pdf-smoke")

from scripts._admin_cli import (  # noqa: E402
    ensure_daemon,
    list_browsers,
    restart_daemon,
    start_remote_daemon,
)
from scripts._runner_cli import goto, page_info, print_pdf, wait_for_load  # noqa: E402

TARGET_URL = "https://example.com/?via=bhrun-print-pdf-smoke"


def poll_browser_status(browser_id, attempts=10, delay=1.0):
    for _ in range(attempts):
        listing = list_browsers(page_size=20, page_number=1)
        item = next((item for item in listing.get("items", []) if item.get("id") == browser_id), None)
        status = item.get("status") if item else "missing"
        if status != "active":
            return status
        time.sleep(delay)
    return status


def decode_pdf(encoded):
    data = base64.b64decode(encoded)
    if not data.startswith(b"%PDF-"):
        raise RuntimeError("runner print-pdf did not return a PDF")
    return data


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
        result["page_before_print"] = page_info(daemon_name=name)

        portrait_encoded = print_pdf(landscape=False, daemon_name=name)
        landscape_encoded = print_pdf(landscape=True, daemon_name=name)
        portrait_pdf = decode_pdf(portrait_encoded)
        landscape_pdf = decode_pdf(landscape_encoded)
        result["portrait_pdf_bytes"] = len(portrait_pdf)
        result["landscape_pdf_bytes"] = len(landscape_pdf)
        result["portrait_prefix"] = portrait_pdf[:8].decode("latin1")
        result["landscape_prefix"] = landscape_pdf[:8].decode("latin1")
        if len(portrait_pdf) < 1000:
            raise RuntimeError("portrait PDF was unexpectedly small")
        if len(landscape_pdf) < 1000:
            raise RuntimeError("landscape PDF was unexpectedly small")
        if portrait_pdf == landscape_pdf:
            raise RuntimeError("portrait and landscape PDFs were identical")

        result["page_after_print"] = page_info(daemon_name=name)
        if result["page_after_print"].get("url") != TARGET_URL:
            raise RuntimeError("page URL changed during print-pdf smoke")
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
