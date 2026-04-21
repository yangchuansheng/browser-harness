#!/usr/bin/env python3
"""Run a live remote-browser smoke test through the Rust daemon path.

Required:
  BROWSER_USE_API_KEY

Optional:
  BU_NAME                   defaults to "remote-smoke"
  BU_DAEMON_IMPL            defaults to "rust"
  BU_REMOTE_TIMEOUT_MINUTES defaults to "1"
  BU_RUST_DAEMON_BIN        override the daemon binary path
"""

import json
import os
import sys
import tempfile
import time
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO))
os.environ.setdefault("BU_NAME", "remote-smoke")

from admin import _browser_use, restart_daemon, start_remote_daemon  # noqa: E402
from helpers import dispatch_key, goto, js, new_tab, page_info, screenshot, wait_for_load  # noqa: E402


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
    if not os.environ.get("BROWSER_USE_API_KEY"):
        raise SystemExit("BROWSER_USE_API_KEY is required")

    os.environ.setdefault("BU_DAEMON_IMPL", "rust")
    name = os.environ["BU_NAME"]
    timeout = int(os.environ.get("BU_REMOTE_TIMEOUT_MINUTES", "1"))

    browser = None
    shot_path = None
    result = {"name": name, "daemon_impl": os.environ["BU_DAEMON_IMPL"]}
    try:
        browser = start_remote_daemon(name=name, timeout=timeout)
        browser_id = browser["id"]
        result["browser_id"] = browser_id
        result["initial_page"] = page_info()
        result["new_tab_target"] = new_tab("https://example.com")
        result["after_new_tab"] = page_info()
        if result["after_new_tab"].get("url") == "about:blank":
            raise RuntimeError("new_tab left the active page at about:blank")
        result["loaded"] = wait_for_load()
        result["url_via_js"] = js("location.href")
        result["goto_result"] = goto("https://example.com/?via=typed-goto")
        result["loaded_after_goto"] = wait_for_load()
        result["after_nav"] = page_info()
        js(
            "(()=>{let e=document.querySelector('#codex-dispatch');"
            "if(!e){e=document.createElement('input');e.id='codex-dispatch';document.body.appendChild(e)}"
            "window.__dispatchKey=null;"
            "e.addEventListener('keypress',ev=>window.__dispatchKey={key:ev.key,which:ev.which,type:ev.type},{once:true});"
            "return true})()"
        )
        dispatch_key("#codex-dispatch", key="Enter", event="keypress")
        result["dispatch_key"] = js("window.__dispatchKey")
        fd, shot_path = tempfile.mkstemp(prefix=f"{name}-", suffix=".png")
        os.close(fd)
        screenshot(shot_path, full=True)
        result["screenshot_size"] = Path(shot_path).stat().st_size
    finally:
        restart_daemon(name)
        time.sleep(1)
        if browser is not None:
            result["post_shutdown_status"] = poll_browser_status(browser["id"])
        log_path = Path(f"/tmp/bu-{name}.log")
        if log_path.exists():
            lines = log_path.read_text().strip().splitlines()
            result["log_tail"] = lines[-8:]
        if shot_path is not None:
            Path(shot_path).unlink(missing_ok=True)

    print(json.dumps(result))


if __name__ == "__main__":
    main()
