"""Legacy compatibility wrappers over runner_cli with raw CDP fallback."""

import json
import os
import socket
import time
from pathlib import Path
from urllib.parse import urlparse

from legacy_warnings import warn_legacy_surface
import runner_cli

click = runner_cli.click
current_tab = runner_cli.current_tab
dispatch_key = runner_cli.dispatch_key
drain_events = runner_cli.drain_events
ensure_real_tab = runner_cli.ensure_real_tab
iframe_target = runner_cli.iframe_target
http_get = runner_cli.http_get
list_tabs = runner_cli.list_tabs
new_tab = runner_cli.new_tab
press_key = runner_cli.press_key
scroll = runner_cli.scroll
screenshot = runner_cli.screenshot
switch_tab = runner_cli.switch_tab
type_text = runner_cli.type_text
upload_file = runner_cli.upload_file
wait = runner_cli.wait_compat


def _load_env():
    path = Path(__file__).parent / ".env"
    if not path.exists():
        return
    for line in path.read_text().splitlines():
        line = line.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        key, value = line.split("=", 1)
        os.environ.setdefault(key.strip(), value.strip().strip('"').strip("'"))


_load_env()
warn_legacy_surface(
    "`import helpers` is deprecated; use `runner_cli` for stable Python helpers or the `browser-harness` CLI for the primary interface."
)

NAME = os.environ.get("BU_NAME", "default")
SOCK = f"/tmp/bu-{NAME}.sock"
INTERNAL = ("chrome://", "chrome-untrusted://", "devtools://", "chrome-extension://", "about:")
_UNSUPPORTED_META = set()
_UNSUPPORTED_MARKER = "unsupported meta command:"


def _send(req):
    sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    sock.connect(SOCK)
    sock.sendall((json.dumps(req) + "\n").encode())
    data = b""
    while not data.endswith(b"\n"):
        chunk = sock.recv(1 << 20)
        if not chunk:
            break
        data += chunk
    sock.close()
    response = json.loads(data)
    if "error" in response:
        raise RuntimeError(response["error"])
    return response


def _runner_or_fallback(meta, runner_call, fallback_call):
    if meta in _UNSUPPORTED_META:
        return fallback_call()
    try:
        return runner_call()
    except RuntimeError as err:
        if _UNSUPPORTED_MARKER in str(err):
            _UNSUPPORTED_META.add(meta)
            return fallback_call()
        raise


def cdp(method, session_id=None, **params):
    """Legacy raw CDP compatibility escape hatch.

    Prefer `browser-harness cdp-raw` or `bh_guest_sdk::cdp_raw(...)` for new
    Rust-native workflows.
    """
    return _send({"method": method, "params": params, "session_id": session_id}).get("result", {})


def goto(url):
    result = _runner_or_fallback(
        "goto",
        lambda: runner_cli.goto(url),
        lambda: cdp("Page.navigate", url=url),
    )
    skill_dir = (
        Path(__file__).parent
        / "domain-skills"
        / (urlparse(url).hostname or "").removeprefix("www.").split(".")[0]
    )
    if skill_dir.is_dir():
        return {
            **result,
            "domain_skills": sorted(path.name for path in skill_dir.rglob("*.md"))[:10],
        }
    return result


def page_info():
    """{url, title, w, h, sx, sy, pw, ph} — viewport + scroll + page size.

    If a native dialog (alert/confirm/prompt/beforeunload) is open, returns
    {dialog: {type, message, ...}} instead — the page's JS thread is frozen
    until the dialog is handled (see interaction-skills/dialogs.md).
    """

    def fallback():
        dialog = _send({"meta": "pending_dialog"}).get("dialog")
        if dialog:
            return {"dialog": dialog}
        result = cdp(
            "Runtime.evaluate",
            expression=(
                "JSON.stringify({url:location.href,title:document.title,w:innerWidth,"
                "h:innerHeight,sx:scrollX,sy:scrollY,pw:document.documentElement.scrollWidth,"
                "ph:document.documentElement.scrollHeight})"
            ),
            returnByValue=True,
        )
        return json.loads(result["result"]["value"])

    return _runner_or_fallback("page_info", lambda: runner_cli.page_info(), fallback)


def wait_for_load(timeout=15.0):
    def fallback():
        deadline = time.time() + timeout
        while time.time() < deadline:
            if js("document.readyState") == "complete":
                return True
            time.sleep(0.3)
        return False

    return _runner_or_fallback(
        "wait_for_load",
        lambda: bool(runner_cli.wait_for_load(timeout=timeout)),
        fallback,
    )


def js(expression, target_id=None):
    """Run JS in the attached tab or inside an iframe target."""

    def fallback():
        session_id = None
        if target_id:
            session_id = cdp("Target.attachToTarget", targetId=target_id, flatten=True)["sessionId"]
        result = cdp(
            "Runtime.evaluate",
            session_id=session_id,
            expression=expression,
            returnByValue=True,
            awaitPromise=True,
        )
        return result.get("result", {}).get("value")

    return _runner_or_fallback(
        "js",
        lambda: runner_cli.js(expression, target_id=target_id),
        fallback,
    )
__all__ = [
    "INTERNAL",
    "NAME",
    "SOCK",
    "cdp",
    "click",
    "current_tab",
    "dispatch_key",
    "drain_events",
    "ensure_real_tab",
    "goto",
    "http_get",
    "iframe_target",
    "js",
    "list_tabs",
    "new_tab",
    "page_info",
    "press_key",
    "screenshot",
    "scroll",
    "switch_tab",
    "type_text",
    "upload_file",
    "wait",
    "wait_for_load",
]
