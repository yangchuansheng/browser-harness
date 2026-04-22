#!/usr/bin/env python3
"""Run a live end-to-end smoke test for `bhrun get-cookies` / `set-cookies`.

Optional:
  BU_NAME                   defaults to "bhrun-cookies-smoke"
  BU_BROWSER_MODE           defaults to "local"; "remote" is best-effort only
  BU_DAEMON_IMPL            defaults to "rust"
  BU_REMOTE_TIMEOUT_MINUTES defaults to "1"
  BU_LOCAL_DAEMON_WAIT_SECONDS defaults to "15"
"""

import json
import os
import sys
import time
import uuid
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO))
os.environ.setdefault("BU_NAME", "bhrun-cookies-smoke")

from admin import _browser_use, ensure_daemon, restart_daemon, start_remote_daemon  # noqa: E402
from scripts._runner_cli import get_cookies, goto, js, page_info, set_cookies, wait_for_load  # noqa: E402

TARGET_URL = "https://example.com/?via=bhrun-cookies-smoke"


def poll_browser_status(browser_id, attempts=10, delay=1.0):
    for _ in range(attempts):
        listing = _browser_use("/browsers?pageSize=20&pageNumber=1", "GET")
        item = next((item for item in listing.get("items", []) if item.get("id") == browser_id), None)
        status = item.get("status") if item else "missing"
        if status != "active":
            return status
        time.sleep(delay)
    return status


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
        result["page_before_cookie"] = page_info(daemon_name=name)

        cookie_name = f"bhrun_cookie_{uuid.uuid4().hex[:12]}"
        cookie_value = uuid.uuid4().hex
        cookie = {
            "name": cookie_name,
            "value": cookie_value,
            "url": TARGET_URL,
            "secure": True,
            "sameSite": "Lax",
        }
        set_cookies([cookie], daemon_name=name)
        result["cookie_set"] = cookie

        visible_cookie = js(
            f"document.cookie.split('; ').find(c => c.startsWith({json.dumps(cookie_name + '=')})) || null",
            daemon_name=name,
        )
        result["document_cookie_entry"] = visible_cookie
        if visible_cookie != f"{cookie_name}={cookie_value}":
            raise RuntimeError(f"document.cookie did not expose the new cookie: {visible_cookie!r}")

        cookies = get_cookies([TARGET_URL], daemon_name=name)
        result["cookie_count"] = len(cookies)
        matched = [cookie for cookie in cookies if cookie.get("name") == cookie_name]
        result["matched_cookies"] = matched
        if len(matched) != 1:
            raise RuntimeError(f"expected exactly one matched cookie, got {len(matched)}")
        if matched[0].get("value") != cookie_value:
            raise RuntimeError("get-cookies returned the wrong cookie value")
        if "example.com" not in matched[0].get("domain", ""):
            raise RuntimeError(f"cookie domain did not contain example.com: {matched[0].get('domain')!r}")

        result["page_after_cookie"] = page_info(daemon_name=name)
        if result["page_after_cookie"].get("url") != TARGET_URL:
            raise RuntimeError("page URL changed during cookie smoke")
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
