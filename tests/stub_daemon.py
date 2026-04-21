#!/usr/bin/env python3
"""Small Unix-socket daemon used by Rust-mode compatibility tests."""

import json
import os
import socket
from pathlib import Path
from urllib.parse import urlparse

MARK = chr(0x1F7E2)
INTERNAL = ("chrome://", "chrome-untrusted://", "devtools://", "chrome-extension://", "about:")


def runtime_paths():
    name = os.environ.get("BU_NAME", "default")
    return (
        name,
        Path(f"/tmp/bu-{name}.sock"),
        Path(f"/tmp/bu-{name}.pid"),
        Path(f"/tmp/bu-{name}.log"),
    )


def log_line(log_path, message):
    with log_path.open("a", encoding="utf-8") as handle:
        handle.write(f"{message}\n")


def normalize_url(url):
    parsed = urlparse(url)
    if not parsed.scheme:
        return url
    path = parsed.path or "/"
    return parsed._replace(path=path).geturl()


def title_for_url(url):
    normalized = normalize_url(url)
    if normalized == "about:blank":
        return ""
    if (urlparse(normalized).hostname or "") == "example.com":
        return "Example Domain"
    host = urlparse(normalized).hostname or normalized
    return host.removeprefix("www.")


class StubState:
    def __init__(self):
        self.dialog = None
        self.events = []
        self.unsupported_meta = {
            meta.strip()
            for meta in os.environ.get("STUB_UNSUPPORTED_META", "").split(",")
            if meta.strip()
        }
        self.current_target = "target-1"
        self.current_session = "session-1"
        self.next_target = 2
        self.next_session = 2
        self.targets = {
            "target-1": {
                "targetId": "target-1",
                "type": "page",
                "url": "about:blank",
                "title": "",
            },
            "iframe-1": {
                "targetId": "iframe-1",
                "type": "iframe",
                "url": "https://frames.example.test/embed",
                "title": "Frame",
            }
        }
        self.sessions = {"session-1": "target-1"}
        self.last_click = None
        self.last_text = None
        self.last_key = None
        self.last_dispatch_key = None
        self.last_scroll = None
        self.uploads = []
        self.should_stop = False

    def active_target(self, session_id=None):
        target_id = self.sessions.get(session_id or self.current_session, self.current_target)
        return self.targets[target_id]

    def attach_target(self, target_id):
        session_id = f"session-{self.next_session}"
        self.next_session += 1
        self.sessions[session_id] = target_id
        return session_id

    def create_target(self, url):
        target_id = f"target-{self.next_target}"
        self.next_target += 1
        self.targets[target_id] = {
            "targetId": target_id,
            "type": "page",
            "url": normalize_url(url),
            "title": title_for_url(url),
        }
        return target_id

    def current_tab(self):
        return self.targets[self.current_target]


def page_info(target):
    return {
        "url": target["url"],
        "title": target["title"],
        "w": 1280,
        "h": 720,
        "sx": 0,
        "sy": 0,
        "pw": 1280,
        "ph": 720,
    }


def evaluate_expression(target, expression):
    if expression == "document.readyState":
        return "complete"
    if expression == "location.href":
        return target["url"]
    if expression == "document.title":
        return target["title"]
    if expression.startswith("JSON.stringify({url:location.href,title:document.title"):
        return json.dumps(page_info(target))
    return None


def handle_meta(state, request, log_path):
    meta = request.get("meta")
    if meta in state.unsupported_meta:
        log_line(log_path, f"unsupported_meta={meta}")
        return {"error": f"unsupported meta command: {meta}"}
    if meta == "drain_events":
        events = state.events
        state.events = []
        return {"events": events}
    if meta == "session":
        return {"session_id": state.current_session}
    if meta == "set_session":
        state.current_session = request.get("session_id")
        return {"session_id": state.current_session}
    if meta == "pending_dialog":
        return {"dialog": state.dialog}
    if meta == "shutdown":
        state.should_stop = True
        return {"ok": True}
    if meta == "page_info":
        log_line(log_path, "typed_meta=page_info")
        if state.dialog:
            return {"result": {"dialog": state.dialog}}
        return {"result": page_info(state.current_tab())}
    if meta == "list_tabs":
        log_line(log_path, "typed_meta=list_tabs")
        include_internal = (request.get("params") or {}).get("include_internal", True)
        tabs = [
            {
                "targetId": target["targetId"],
                "title": target["title"],
                "url": target["url"],
            }
            for target in state.targets.values()
            if target["type"] == "page" and (include_internal or not target["url"].startswith("about:"))
        ]
        return {"result": tabs}
    if meta == "current_tab":
        log_line(log_path, "typed_meta=current_tab")
        current = state.current_tab()
        return {
            "result": {
                "targetId": current["targetId"],
                "title": current["title"],
                "url": current["url"],
            }
        }
    if meta == "switch_tab":
        log_line(log_path, "typed_meta=switch_tab")
        target_id = (request.get("params") or {}).get("target_id")
        state.current_target = target_id
        state.current_session = state.attach_target(target_id)
        current = state.current_tab()
        if not current["title"].startswith(MARK):
            current["title"] = f"{MARK} {current['title']}".rstrip()
        return {"result": state.current_session}
    if meta == "new_tab":
        log_line(log_path, "typed_meta=new_tab")
        url = (request.get("params") or {}).get("url", "about:blank")
        target_id = state.create_target("about:blank")
        state.current_target = target_id
        state.current_session = state.attach_target(target_id)
        if url != "about:blank":
            current = state.current_tab()
            current["url"] = normalize_url(url)
            current["title"] = f"{MARK} {title_for_url(url)}".rstrip()
        return {"result": target_id}
    if meta == "ensure_real_tab":
        log_line(log_path, "typed_meta=ensure_real_tab")
        tabs = [
            {
                "targetId": target["targetId"],
                "title": target["title"],
                "url": target["url"],
            }
            for target in state.targets.values()
            if target["type"] == "page" and not target["url"].startswith(INTERNAL)
        ]
        if not tabs:
            return {"result": None}
        current = state.current_tab()
        if current["url"] and not current["url"].startswith(INTERNAL):
            return {"result": tabs[0] if current["targetId"] == tabs[0]["targetId"] else {
                "targetId": current["targetId"],
                "title": current["title"],
                "url": current["url"],
            }}
        state.current_target = tabs[0]["targetId"]
        state.current_session = state.attach_target(state.current_target)
        return {"result": tabs[0]}
    if meta == "iframe_target":
        log_line(log_path, "typed_meta=iframe_target")
        url_substr = (request.get("params") or {}).get("url_substr", "")
        for target in state.targets.values():
            if target["type"] == "iframe" and url_substr in target["url"]:
                return {"result": target["targetId"]}
        return {"result": None}
    if meta == "wait_for_load":
        log_line(log_path, "typed_meta=wait_for_load")
        return {"result": True}
    if meta == "goto":
        log_line(log_path, "typed_meta=goto")
        url = (request.get("params") or {}).get("url", "about:blank")
        current = state.current_tab()
        current["url"] = normalize_url(url)
        current["title"] = title_for_url(url)
        return {"result": {"frameId": "frame-1"}}
    if meta == "js":
        log_line(log_path, "typed_meta=js")
        params = request.get("params") or {}
        expression = params.get("expression", "")
        target = state.targets.get(params.get("target_id")) if params.get("target_id") else state.current_tab()
        if target is None:
            return {"result": None}
        if "document.title.startsWith" in expression and "document.title=" in expression:
            if not target["title"].startswith(MARK):
                target["title"] = f"{MARK} {target['title']}".rstrip()
            return {"result": None}
        if "document.title.startsWith" in expression and "slice(2)" in expression:
            target["title"] = target["title"].removeprefix(f"{MARK} ")
            return {"result": None}
        return {"result": evaluate_expression(target, expression)}
    if meta == "screenshot":
        log_line(log_path, "typed_meta=screenshot")
        return {"result": "c3R1Yi1zaG90"}
    if meta == "click":
        log_line(log_path, "typed_meta=click")
        state.last_click = dict(request.get("params") or {})
        return {"result": None}
    if meta == "type_text":
        log_line(log_path, "typed_meta=type_text")
        state.last_text = (request.get("params") or {}).get("text")
        return {"result": None}
    if meta == "press_key":
        log_line(log_path, "typed_meta=press_key")
        state.last_key = dict(request.get("params") or {})
        return {"result": None}
    if meta == "dispatch_key":
        log_line(log_path, "typed_meta=dispatch_key")
        state.last_dispatch_key = dict(request.get("params") or {})
        return {"result": None}
    if meta == "scroll":
        log_line(log_path, "typed_meta=scroll")
        state.last_scroll = dict(request.get("params") or {})
        return {"result": None}
    if meta == "upload_file":
        log_line(log_path, "typed_meta=upload_file")
        params = request.get("params") or {}
        if params.get("selector") != "#file1":
            return {"error": f"no element for {params.get('selector')}"}
        state.uploads.append(
            {
                "selector": params.get("selector"),
                "files": list(params.get("files") or []),
                "target_id": params.get("target_id"),
            }
        )
        return {"result": None}
    return {"error": f"unsupported meta command: {meta}"}


def handle_runtime_evaluate(state, request):
    expression = request.get("params", {}).get("expression", "")
    target = state.active_target(request.get("session_id"))
    value = evaluate_expression(target, expression)
    if value is not None:
        return {"result": {"result": {"value": value}}}
    if "document.title.startsWith" in expression and "document.title=" in expression:
        if not target["title"].startswith(MARK):
            target["title"] = f"{MARK} {target['title']}".rstrip()
        return {"result": {"result": {"value": None}}}
    if "document.title.startsWith" in expression and "slice(2)" in expression:
        target["title"] = target["title"].removeprefix(f"{MARK} ")
        return {"result": {"result": {"value": None}}}
    return {"result": {"result": {"value": None}}}


def handle_request(state, request, log_path):
    if request.get("meta"):
        return handle_meta(state, request, log_path)

    method = request.get("method")
    params = request.get("params") or {}
    log_line(log_path, f"raw_method={method}")

    if method == "Target.getTargets":
        return {"result": {"targetInfos": list(state.targets.values())}}
    if method == "Target.getTargetInfo":
        return {"result": {"targetInfo": state.active_target(request.get("session_id"))}}
    if method == "Target.createTarget":
        return {"result": {"targetId": state.create_target(params.get("url", "about:blank"))}}
    if method == "Target.activateTarget":
        state.current_target = params["targetId"]
        return {"result": {}}
    if method == "Target.attachToTarget":
        return {"result": {"sessionId": state.attach_target(params["targetId"])}}
    if method == "Page.navigate":
        target = state.active_target(request.get("session_id"))
        target["url"] = normalize_url(params["url"])
        target["title"] = title_for_url(params["url"])
        return {"result": {"frameId": "frame-1"}}
    if method == "Runtime.evaluate":
        return handle_runtime_evaluate(state, request)
    return {"result": {}}


def serve():
    name, sock_path, pid_path, log_path = runtime_paths()
    sock_path.unlink(missing_ok=True)
    pid_path.write_text(str(os.getpid()), encoding="utf-8")
    log_path.write_text("", encoding="utf-8")
    log_line(log_path, f"started name={name}")
    log_line(log_path, f"env_STUB_GREETING={os.environ.get('STUB_GREETING', '')}")

    state = StubState()
    server = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    try:
        server.bind(str(sock_path))
        server.listen()
        server.settimeout(0.2)
        while not state.should_stop:
            try:
                conn, _ = server.accept()
            except socket.timeout:
                continue
            with conn:
                data = b""
                while not data.endswith(b"\n"):
                    chunk = conn.recv(1 << 20)
                    if not chunk:
                        break
                    data += chunk
                if not data:
                    continue
                request = json.loads(data)
                response = handle_request(state, request, log_path)
                conn.sendall((json.dumps(response) + "\n").encode())
    except Exception as exc:
        log_line(log_path, f"fatal: {exc}")
        raise
    finally:
        server.close()
        sock_path.unlink(missing_ok=True)
        pid_path.unlink(missing_ok=True)


if __name__ == "__main__":
    serve()
