import base64
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent
_DEFAULT_SCREENSHOT_PATH = object()

__all__ = [
    "cdp_raw",
    "click",
    "configure_downloads",
    "current_session",
    "current_tab",
    "dispatch_key",
    "drain_events",
    "ensure_real_tab",
    "get_cookies",
    "goto",
    "handle_dialog",
    "http_get",
    "iframe_target",
    "js",
    "list_tabs",
    "mouse_down",
    "mouse_move",
    "mouse_up",
    "new_tab",
    "page_info",
    "press_key",
    "print_pdf",
    "run_runner_command",
    "runner_process_spec",
    "scroll",
    "screenshot",
    "set_cookies",
    "set_viewport",
    "switch_tab",
    "type_text",
    "upload_file",
    "wait",
    "wait_for_download",
    "wait_for_load",
    "wait_for_request",
]


def _load_env():
    path = REPO / ".env"
    if not path.exists():
        return
    for line in path.read_text().splitlines():
        line = line.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        key, value = line.split("=", 1)
        os.environ.setdefault(key.strip(), value.strip().strip('"').strip("'"))


_load_env()


def _daemon_name(daemon_name=None):
    return daemon_name or os.environ.get("BU_NAME", "default")


def _installed_bhrun():
    candidates = [
        Path(sys.argv[0]).resolve().with_name("bhrun"),
        Path(sys.executable).resolve().with_name("bhrun"),
    ]
    for candidate in candidates:
        if candidate.is_file():
            return str(candidate)
    return shutil.which("bhrun")


def runner_process_spec():
    if custom := os.environ.get("BU_RUST_RUNNER_BIN"):
        return [custom], str(REPO)
    if installed := _installed_bhrun():
        return [installed], str(REPO)
    return ["cargo", "run", "--quiet", "--bin", "bhrun", "--"], str(REPO / "rust")


def run_runner_command(subcommand, payload=None, timeout_seconds=15, extra_args=None):
    cmd, cwd = runner_process_spec()
    proc = subprocess.Popen(
        cmd + [subcommand] + (extra_args or []),
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


def _named_runner_command(subcommand, daemon_name=None, timeout_seconds=15, **payload):
    request = {"daemon_name": _daemon_name(daemon_name)}
    request.update(payload)
    return run_runner_command(subcommand, request, timeout_seconds=timeout_seconds)


def cdp_raw(method, params=None, session_id=None, daemon_name=None, timeout_seconds=15):
    payload = {
        "daemon_name": _daemon_name(daemon_name),
        "method": method,
    }
    if params is not None:
        payload["params"] = params
    if session_id is not None:
        payload["session_id"] = session_id
    result, _ = run_runner_command(
        "cdp-raw",
        payload,
        timeout_seconds=timeout_seconds,
    )
    return result


def current_session(daemon_name=None, timeout_seconds=10):
    result, _ = _named_runner_command(
        "current-session",
        daemon_name=daemon_name,
        timeout_seconds=timeout_seconds,
    )
    return result


def drain_events(daemon_name=None, timeout_seconds=10):
    result, _ = _named_runner_command(
        "drain-events",
        daemon_name=daemon_name,
        timeout_seconds=timeout_seconds,
    )
    return result


def current_tab(daemon_name=None, timeout_seconds=10):
    result, _ = _named_runner_command(
        "current-tab",
        daemon_name=daemon_name,
        timeout_seconds=timeout_seconds,
    )
    return result


def list_tabs(include_internal=True, include_chrome=None, daemon_name=None, timeout_seconds=10):
    normalized_include_internal = include_chrome if include_chrome is not None else include_internal
    if not isinstance(normalized_include_internal, bool):
        raise TypeError("include_internal/include_chrome must be a bool")
    result, _ = _named_runner_command(
        "list-tabs",
        daemon_name=daemon_name,
        timeout_seconds=timeout_seconds,
        include_internal=normalized_include_internal,
    )
    return result


def new_tab(url="about:blank", daemon_name=None, timeout_seconds=10):
    result, _ = _named_runner_command(
        "new-tab",
        daemon_name=daemon_name,
        timeout_seconds=timeout_seconds,
        url=url,
    )
    return result.get("target_id")


def switch_tab(target_id, daemon_name=None, timeout_seconds=10):
    result, _ = _named_runner_command(
        "switch-tab",
        daemon_name=daemon_name,
        timeout_seconds=timeout_seconds,
        target_id=target_id,
    )
    return result.get("session_id")


def ensure_real_tab(daemon_name=None, timeout_seconds=10):
    result, _ = _named_runner_command(
        "ensure-real-tab",
        daemon_name=daemon_name,
        timeout_seconds=timeout_seconds,
    )
    return result


def iframe_target(url_substr, daemon_name=None, timeout_seconds=10):
    result, _ = _named_runner_command(
        "iframe-target",
        daemon_name=daemon_name,
        timeout_seconds=timeout_seconds,
        url_substr=url_substr,
    )
    return result


def page_info(daemon_name=None, timeout_seconds=10):
    result, _ = _named_runner_command(
        "page-info",
        daemon_name=daemon_name,
        timeout_seconds=timeout_seconds,
    )
    return result


def goto(url, daemon_name=None, timeout_seconds=15):
    result, _ = _named_runner_command(
        "goto",
        daemon_name=daemon_name,
        timeout_seconds=timeout_seconds,
        url=url,
    )
    return result


def wait_for_load(timeout=15.0, daemon_name=None, timeout_seconds=None):
    result, _ = _named_runner_command(
        "wait-for-load",
        daemon_name=daemon_name,
        timeout_seconds=timeout_seconds or max(15, int(timeout) + 5),
        timeout=timeout,
    )
    return bool(result)


def js(expression, target_id=None, daemon_name=None, timeout_seconds=15):
    payload = {"expression": expression}
    if target_id is not None:
        payload["target_id"] = target_id
    result, _ = _named_runner_command(
        "js",
        daemon_name=daemon_name,
        timeout_seconds=timeout_seconds,
        **payload,
    )
    return result


def click(x, y, button="left", clicks=1, daemon_name=None, timeout_seconds=10):
    result, _ = _named_runner_command(
        "click",
        daemon_name=daemon_name,
        timeout_seconds=timeout_seconds,
        x=x,
        y=y,
        button=button,
        clicks=clicks,
    )
    return result


def mouse_move(x, y, buttons=0, daemon_name=None, timeout_seconds=10):
    result, _ = _named_runner_command(
        "mouse-move",
        daemon_name=daemon_name,
        timeout_seconds=timeout_seconds,
        x=x,
        y=y,
        buttons=buttons,
    )
    return result


def mouse_down(x, y, button="left", buttons=1, click_count=1, daemon_name=None, timeout_seconds=10):
    result, _ = _named_runner_command(
        "mouse-down",
        daemon_name=daemon_name,
        timeout_seconds=timeout_seconds,
        x=x,
        y=y,
        button=button,
        buttons=buttons,
        click_count=click_count,
    )
    return result


def mouse_up(x, y, button="left", buttons=0, click_count=1, daemon_name=None, timeout_seconds=10):
    result, _ = _named_runner_command(
        "mouse-up",
        daemon_name=daemon_name,
        timeout_seconds=timeout_seconds,
        x=x,
        y=y,
        button=button,
        buttons=buttons,
        click_count=click_count,
    )
    return result


def type_text(text, daemon_name=None, timeout_seconds=10):
    result, _ = _named_runner_command(
        "type-text",
        daemon_name=daemon_name,
        timeout_seconds=timeout_seconds,
        text=text,
    )
    return result


def press_key(key, modifiers=0, daemon_name=None, timeout_seconds=10):
    result, _ = _named_runner_command(
        "press-key",
        daemon_name=daemon_name,
        timeout_seconds=timeout_seconds,
        key=key,
        modifiers=modifiers,
    )
    return result


def dispatch_key(selector, key="Enter", event="keypress", daemon_name=None, timeout_seconds=10):
    result, _ = _named_runner_command(
        "dispatch-key",
        daemon_name=daemon_name,
        timeout_seconds=timeout_seconds,
        selector=selector,
        key=key,
        event=event,
    )
    return result


def scroll(x, y, dy=-300, dx=0, daemon_name=None, timeout_seconds=10):
    result, _ = _named_runner_command(
        "scroll",
        daemon_name=daemon_name,
        timeout_seconds=timeout_seconds,
        x=x,
        y=y,
        dy=dy,
        dx=dx,
    )
    return result


def set_viewport(
    width,
    height,
    device_scale_factor=1.0,
    mobile=False,
    daemon_name=None,
    timeout_seconds=10,
):
    result, _ = _named_runner_command(
        "set-viewport",
        daemon_name=daemon_name,
        timeout_seconds=timeout_seconds,
        width=width,
        height=height,
        device_scale_factor=device_scale_factor,
        mobile=mobile,
    )
    return result


def screenshot(path=_DEFAULT_SCREENSHOT_PATH, full=False, daemon_name=None, timeout_seconds=20):
    encoded, _ = _named_runner_command(
        "screenshot",
        daemon_name=daemon_name,
        timeout_seconds=timeout_seconds,
        full=full,
    )
    if path is _DEFAULT_SCREENSHOT_PATH:
        path = "/tmp/shot.png"
    if path is None:
        return encoded
    Path(path).write_bytes(base64.b64decode(encoded))
    return path


def print_pdf(path=None, landscape=False, daemon_name=None, timeout_seconds=20):
    encoded, _ = _named_runner_command(
        "print-pdf",
        daemon_name=daemon_name,
        timeout_seconds=timeout_seconds,
        landscape=landscape,
    )
    if path is None:
        return encoded
    Path(path).write_bytes(base64.b64decode(encoded))
    return path


def get_cookies(urls=None, daemon_name=None, timeout_seconds=10):
    payload = {}
    if urls is not None:
        payload["urls"] = urls
    result, _ = _named_runner_command(
        "get-cookies",
        daemon_name=daemon_name,
        timeout_seconds=timeout_seconds,
        **payload,
    )
    return result


def set_cookies(cookies, daemon_name=None, timeout_seconds=10):
    result, _ = _named_runner_command(
        "set-cookies",
        daemon_name=daemon_name,
        timeout_seconds=timeout_seconds,
        cookies=cookies,
    )
    return result


def configure_downloads(download_path, daemon_name=None, timeout_seconds=10):
    result, _ = _named_runner_command(
        "configure-downloads",
        daemon_name=daemon_name,
        timeout_seconds=timeout_seconds,
        download_path=download_path,
    )
    return result


def handle_dialog(action="accept", prompt_text=None, daemon_name=None, timeout_seconds=10):
    payload = {"action": action}
    if prompt_text is not None:
        payload["prompt_text"] = prompt_text
    result, _ = _named_runner_command(
        "handle-dialog",
        daemon_name=daemon_name,
        timeout_seconds=timeout_seconds,
        **payload,
    )
    return result


def upload_file(selector, files, target_id=None, daemon_name=None, timeout_seconds=10):
    normalized_files = [files] if isinstance(files, str) else list(files)
    payload = {
        "selector": selector,
        "files": normalized_files,
    }
    if target_id is not None:
        payload["target_id"] = target_id
    result, _ = _named_runner_command(
        "upload-file",
        daemon_name=daemon_name,
        timeout_seconds=timeout_seconds,
        **payload,
    )
    return result


def wait(seconds=1.0, timeout_seconds=None):
    duration_ms = max(0, int(seconds * 1000))
    result, _ = run_runner_command(
        "wait",
        {"duration_ms": duration_ms},
        timeout_seconds=timeout_seconds or max(10, int(seconds) + 5),
    )
    return result


def wait_compat(seconds=1.0, timeout_seconds=None):
    wait(seconds, timeout_seconds=timeout_seconds)
    return None


def wait_for_request(
    url,
    method=None,
    session_id=None,
    daemon_name=None,
    timeout_ms=5000,
    poll_interval_ms=100,
    timeout_seconds=15,
):
    payload = {
        "daemon_name": _daemon_name(daemon_name),
        "url": url,
        "timeout_ms": timeout_ms,
        "poll_interval_ms": poll_interval_ms,
    }
    if method is not None:
        payload["method"] = method
    if session_id is not None:
        payload["session_id"] = session_id
    result, _ = run_runner_command(
        "wait-for-request",
        payload,
        timeout_seconds=timeout_seconds,
    )
    return result


def wait_for_download(
    filename=None,
    url=None,
    daemon_name=None,
    timeout_ms=5000,
    poll_interval_ms=100,
    timeout_seconds=15,
):
    payload = {
        "daemon_name": _daemon_name(daemon_name),
        "timeout_ms": timeout_ms,
        "poll_interval_ms": poll_interval_ms,
    }
    if filename is not None:
        payload["filename"] = filename
    if url is not None:
        payload["url"] = url
    result, _ = run_runner_command(
        "wait-for-download",
        payload,
        timeout_seconds=timeout_seconds,
    )
    return result


def http_get(url, headers=None, timeout=20.0, timeout_seconds=None):
    payload = {"url": url, "timeout": timeout}
    if headers:
        payload["headers"] = headers
    result, _ = run_runner_command(
        "http-get",
        payload,
        timeout_seconds=timeout_seconds or max(20, int(timeout) + 5),
    )
    return result
