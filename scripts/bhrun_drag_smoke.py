#!/usr/bin/env python3
"""Run a live end-to-end smoke test for low-level pointer drag primitives.

Optional:
  BU_NAME                   defaults to "bhrun-drag-smoke"
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
os.environ.setdefault("BU_NAME", "bhrun-drag-smoke")

from admin import _browser_use, ensure_daemon, restart_daemon, start_remote_daemon  # noqa: E402
from scripts._runner_cli import (  # noqa: E402
    goto,
    js,
    mouse_down,
    mouse_move,
    mouse_up,
    page_info,
    set_viewport,
    wait,
    wait_for_load,
)


def poll_browser_status(browser_id, attempts=10, delay=1.0):
    for _ in range(attempts):
        listing = _browser_use("/browsers?pageSize=20&pageNumber=1", "GET")
        item = next((item for item in listing.get("items", []) if item.get("id") == browser_id), None)
        status = item.get("status") if item else "missing"
        if status != "active":
            return status
        time.sleep(delay)
    return status


def install_drag_fixture(name):
    return js(
        f"""
(() => {{
  document.title = {json.dumps(name)};
  document.body.innerHTML = `
    <style>
      body {{ margin: 0; font-family: monospace; background: #f6f2e8; }}
      #track {{ position: absolute; left: 80px; top: 160px; width: 520px; height: 24px;
                background: #d0c8b6; border-radius: 999px; }}
      #fill {{ position: absolute; left: 0; top: 0; height: 24px; width: 0;
               background: #2f7a6b; border-radius: 999px; }}
      #handle {{ position: absolute; left: 0; top: -8px; width: 40px; height: 40px;
                 background: #0a5f7a; border-radius: 999px; box-shadow: 0 4px 12px rgba(0,0,0,.18); }}
      #status {{ position: absolute; left: 80px; top: 230px; }}
    </style>
    <div id="track"><div id="fill"></div><div id="handle"></div></div>
    <pre id="status"></pre>
  `;

  const track = document.getElementById('track');
  const fill = document.getElementById('fill');
  const handle = document.getElementById('handle');
  const status = document.getElementById('status');
  const state = {{ events: [], finalLeft: 0, dragging: false }};
  window.__dragState = state;

  let offsetX = 0;
  const maxLeft = () => track.clientWidth - handle.offsetWidth;
  const clamp = (value) => Math.max(0, Math.min(maxLeft(), value));
  const sync = (left) => {{
    handle.style.left = `${{left}}px`;
    fill.style.width = `${{left + handle.offsetWidth / 2}}px`;
    state.finalLeft = left;
    status.textContent = JSON.stringify(state);
  }};

  handle.addEventListener('mousedown', (event) => {{
    state.dragging = true;
    offsetX = event.clientX - handle.getBoundingClientRect().left;
    state.events.push({{ type: 'down', x: event.clientX, y: event.clientY, buttons: event.buttons }});
    sync(state.finalLeft);
    event.preventDefault();
  }});

  document.addEventListener('mousemove', (event) => {{
    if (!state.dragging) return;
    const nextLeft = clamp(event.clientX - track.getBoundingClientRect().left - offsetX);
    state.events.push({{
      type: 'move',
      x: event.clientX,
      y: event.clientY,
      buttons: event.buttons,
      left: nextLeft
    }});
    sync(nextLeft);
  }});

  document.addEventListener('mouseup', (event) => {{
    if (!state.dragging) return;
    state.dragging = false;
    state.events.push({{ type: 'up', x: event.clientX, y: event.clientY, buttons: event.buttons }});
    sync(state.finalLeft);
  }});

  sync(0);
  const handleRect = handle.getBoundingClientRect();
  const trackRect = track.getBoundingClientRect();
  return {{
    startX: handleRect.left + handleRect.width / 2,
    startY: handleRect.top + handleRect.height / 2,
    midX: trackRect.left + trackRect.width * 0.55,
    endX: trackRect.left + trackRect.width - handleRect.width / 2 - 8,
    endY: handleRect.top + handleRect.height / 2,
    maxLeft: maxLeft()
  }};
}})()
""",
        daemon_name=name,
    )


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
    }
    try:
        if browser_mode == "remote":
            browser = start_remote_daemon(name=name, timeout=timeout)
            result["browser_id"] = browser["id"]
        else:
            ensure_daemon(name=name, wait=local_wait)
            result["local_attach"] = "DevToolsActivePort"

        goto("about:blank", daemon_name=name)
        result["loaded"] = wait_for_load(daemon_name=name)
        set_viewport(900, 700, daemon_name=name)
        wait(0.2)
        result["page_before_drag"] = page_info(daemon_name=name)
        result["fixture_geometry"] = install_drag_fixture(name)

        geom = result["fixture_geometry"]
        mouse_move(geom["startX"], geom["startY"], buttons=0, daemon_name=name)
        wait(0.05)
        mouse_down(geom["startX"], geom["startY"], button="left", buttons=1, click_count=1, daemon_name=name)
        wait(0.05)
        mouse_move(geom["midX"], geom["endY"], buttons=1, daemon_name=name)
        wait(0.05)
        mouse_move(geom["endX"], geom["endY"], buttons=1, daemon_name=name)
        wait(0.05)
        mouse_up(geom["endX"], geom["endY"], button="left", buttons=0, click_count=1, daemon_name=name)
        wait(0.1)

        drag_state = js("window.__dragState", daemon_name=name)
        result["drag_state"] = drag_state
        result["page_after_drag"] = page_info(daemon_name=name)

        events = drag_state.get("events") or []
        event_types = [event.get("type") for event in events]
        result["event_types"] = event_types
        if event_types[:1] != ["down"] or "move" not in event_types or event_types[-1:] != ["up"]:
            raise RuntimeError(f"unexpected drag event sequence: {event_types!r}")
        if drag_state.get("finalLeft", 0) < geom["maxLeft"] * 0.65:
            raise RuntimeError(
                f"drag did not move far enough: {drag_state.get('finalLeft')} vs {geom['maxLeft']}"
            )
        if drag_state.get("dragging"):
            raise RuntimeError("drag state stayed active after mouse_up")
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
