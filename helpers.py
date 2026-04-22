"""Legacy compatibility wrappers over runner_cli with raw CDP fallback."""

import base64
import json
import os
import socket
import time
from pathlib import Path
from urllib.parse import urlparse

import runner_cli


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


def drain_events():
    return _runner_or_fallback(
        "drain_events",
        lambda: runner_cli.drain_events(),
        lambda: _send({"meta": "drain_events"})["events"],
    )


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


def click(x, y, button="left", clicks=1):
    return _runner_or_fallback(
        "click",
        lambda: runner_cli.click(x, y, button=button, clicks=clicks),
        lambda: (
            cdp(
                "Input.dispatchMouseEvent",
                type="mousePressed",
                x=x,
                y=y,
                button=button,
                clickCount=clicks,
            ),
            cdp(
                "Input.dispatchMouseEvent",
                type="mouseReleased",
                x=x,
                y=y,
                button=button,
                clickCount=clicks,
            ),
        )[-1],
    )


def type_text(text):
    return _runner_or_fallback(
        "type_text",
        lambda: runner_cli.type_text(text),
        lambda: cdp("Input.insertText", text=text),
    )


_KEYS = {
    "Enter": (13, "Enter", "\r"),
    "Tab": (9, "Tab", "\t"),
    "Backspace": (8, "Backspace", ""),
    "Escape": (27, "Escape", ""),
    "Delete": (46, "Delete", ""),
    " ": (32, "Space", " "),
    "ArrowLeft": (37, "ArrowLeft", ""),
    "ArrowUp": (38, "ArrowUp", ""),
    "ArrowRight": (39, "ArrowRight", ""),
    "ArrowDown": (40, "ArrowDown", ""),
    "Home": (36, "Home", ""),
    "End": (35, "End", ""),
    "PageUp": (33, "PageUp", ""),
    "PageDown": (34, "PageDown", ""),
}


def press_key(key, modifiers=0):
    """Modifiers bitfield: 1=Alt, 2=Ctrl, 4=Meta(Cmd), 8=Shift."""

    def fallback():
        vk, code, text = _KEYS.get(
            key,
            (ord(key[0]) if len(key) == 1 else 0, key, key if len(key) == 1 else ""),
        )
        base = {
            "key": key,
            "code": code,
            "modifiers": modifiers,
            "windowsVirtualKeyCode": vk,
            "nativeVirtualKeyCode": vk,
        }
        cdp(
            "Input.dispatchKeyEvent",
            type="keyDown",
            **base,
            **({"text": text} if text else {}),
        )
        if text and len(text) == 1:
            cdp(
                "Input.dispatchKeyEvent",
                type="char",
                text=text,
                **{name: value for name, value in base.items() if name != "text"},
            )
        return cdp("Input.dispatchKeyEvent", type="keyUp", **base)

    return _runner_or_fallback(
        "press_key",
        lambda: runner_cli.press_key(key, modifiers=modifiers),
        fallback,
    )


def scroll(x, y, dy=-300, dx=0):
    return _runner_or_fallback(
        "scroll",
        lambda: runner_cli.scroll(x, y, dy=dy, dx=dx),
        lambda: cdp(
            "Input.dispatchMouseEvent",
            type="mouseWheel",
            x=x,
            y=y,
            deltaX=dx,
            deltaY=dy,
        ),
    )


def screenshot(path="/tmp/shot.png", full=False):
    def fallback():
        data = cdp("Page.captureScreenshot", format="png", captureBeyondViewport=full)["data"]
        Path(path).write_bytes(base64.b64decode(data))
        return path

    return _runner_or_fallback(
        "screenshot",
        lambda: runner_cli.screenshot(path=path, full=full),
        fallback,
    )


def list_tabs(include_chrome=True):
    return _runner_or_fallback(
        "list_tabs",
        lambda: runner_cli.list_tabs(include_internal=include_chrome),
        lambda: [
            {"targetId": tab["targetId"], "title": tab.get("title", ""), "url": tab.get("url", "")}
            for tab in cdp("Target.getTargets")["targetInfos"]
            if tab["type"] == "page"
            and (include_chrome or not tab.get("url", "").startswith(INTERNAL))
        ],
    )


def current_tab():
    return _runner_or_fallback(
        "current_tab",
        lambda: runner_cli.current_tab(),
        lambda: {
            "targetId": cdp("Target.getTargetInfo").get("targetInfo", {}).get("targetId"),
            "url": cdp("Target.getTargetInfo").get("targetInfo", {}).get("url", ""),
            "title": cdp("Target.getTargetInfo").get("targetInfo", {}).get("title", ""),
        },
    )


def _mark_tab():
    try:
        cdp(
            "Runtime.evaluate",
            expression="if(!document.title.startsWith('\\U0001F7E2'))document.title='\\U0001F7E2 '+document.title",
        )
    except Exception:
        pass


def switch_tab(target_id):
    def fallback():
        try:
            cdp(
                "Runtime.evaluate",
                expression="if(document.title.startsWith('\\U0001F7E2 '))document.title=document.title.slice(2)",
            )
        except Exception:
            pass
        cdp("Target.activateTarget", targetId=target_id)
        session_id = cdp("Target.attachToTarget", targetId=target_id, flatten=True)["sessionId"]
        _send({"meta": "set_session", "session_id": session_id})
        _mark_tab()
        return session_id

    return _runner_or_fallback(
        "switch_tab",
        lambda: runner_cli.switch_tab(target_id),
        fallback,
    )


def new_tab(url="about:blank"):
    def fallback():
        target_id = cdp("Target.createTarget", url="about:blank")["targetId"]
        switch_tab(target_id)
        if url != "about:blank":
            goto(url)
            deadline = time.time() + 5.0
            while time.time() < deadline:
                if js("location.href") != "about:blank":
                    break
                time.sleep(0.1)
            else:
                raise RuntimeError("new_tab navigation did not start before timeout")
        return target_id

    return _runner_or_fallback(
        "new_tab",
        lambda: runner_cli.new_tab(url=url),
        fallback,
    )


def ensure_real_tab():
    """Switch to a real user tab if current is chrome:// / internal / stale."""

    def fallback():
        tabs = list_tabs(include_chrome=False)
        if not tabs:
            return None
        try:
            current = current_tab()
            if current["url"] and not current["url"].startswith(INTERNAL):
                return current
        except Exception:
            pass
        switch_tab(tabs[0]["targetId"])
        return tabs[0]

    return _runner_or_fallback(
        "ensure_real_tab",
        lambda: runner_cli.ensure_real_tab(),
        fallback,
    )


def iframe_target(url_substr):
    """First iframe target whose URL contains `url_substr`."""

    return _runner_or_fallback(
        "iframe_target",
        lambda: runner_cli.iframe_target(url_substr),
        lambda: next(
            (
                tab["targetId"]
                for tab in cdp("Target.getTargets")["targetInfos"]
                if tab["type"] == "iframe" and url_substr in tab.get("url", "")
            ),
            None,
        ),
    )


def wait(seconds=1.0):
    """Client-side sleep utility intentionally kept outside the daemon contract."""

    time.sleep(seconds)


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


_KC = {
    "Enter": 13,
    "Tab": 9,
    "Escape": 27,
    "Backspace": 8,
    " ": 32,
    "ArrowLeft": 37,
    "ArrowUp": 38,
    "ArrowRight": 39,
    "ArrowDown": 40,
}


def dispatch_key(selector, key="Enter", event="keypress"):
    """Dispatch a DOM KeyboardEvent on the matched element."""

    def fallback():
        key_code = _KC.get(key, ord(key) if len(key) == 1 else 0)
        return js(
            "(()=>{const e=document.querySelector("
            + json.dumps(selector)
            + ");if(e){e.focus();e.dispatchEvent(new KeyboardEvent("
            + json.dumps(event)
            + ",{key:"
            + json.dumps(key)
            + ",code:"
            + json.dumps(key)
            + f",keyCode:{key_code},which:{key_code},bubbles:true}}));}})()"
        )

    return _runner_or_fallback(
        "dispatch_key",
        lambda: runner_cli.dispatch_key(selector, key=key, event=event),
        fallback,
    )


def upload_file(selector, path, target_id=None):
    """Set files on a file input via CDP DOM.setFileInputFiles."""

    files = [path] if isinstance(path, str) else list(path)

    def fallback():
        session_id = cdp("Target.attachToTarget", targetId=target_id, flatten=True)["sessionId"] if target_id else None
        try:
            document = cdp("DOM.getDocument", session_id=session_id, depth=-1)
            node_id = cdp(
                "DOM.querySelector",
                session_id=session_id,
                nodeId=document["root"]["nodeId"],
                selector=selector,
            )["nodeId"]
            if not node_id:
                raise RuntimeError(f"no element for {selector}")
            return cdp("DOM.setFileInputFiles", session_id=session_id, files=files, nodeId=node_id)
        finally:
            if session_id:
                try:
                    cdp("Target.detachFromTarget", sessionId=session_id)
                except Exception:
                    pass

    return _runner_or_fallback(
        "upload_file",
        lambda: runner_cli.upload_file(selector, files, target_id=target_id),
        fallback,
    )


def http_get(url, headers=None, timeout=20.0):
    """Pure HTTP utility now routed through the Rust runner."""

    return runner_cli.http_get(url, headers=headers, timeout=timeout)
