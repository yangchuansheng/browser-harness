import base64
import json
import os
import subprocess
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]


def _daemon_name(daemon_name=None):
    return daemon_name or os.environ.get("BU_NAME", "default")


def runner_process_spec():
    if custom := os.environ.get("BU_RUST_RUNNER_BIN"):
        return [custom], str(REPO)
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


def page_info(daemon_name=None, timeout_seconds=10):
    result, _ = _named_runner_command(
        "page-info",
        daemon_name=daemon_name,
        timeout_seconds=timeout_seconds,
    )
    return result


def new_tab(url=None, daemon_name=None, timeout_seconds=10):
    payload = {}
    if url is not None:
        payload["url"] = url
    result, _ = _named_runner_command(
        "new-tab",
        daemon_name=daemon_name,
        timeout_seconds=timeout_seconds,
        **payload,
    )
    return result.get("target_id")


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


def screenshot(path=None, full=False, daemon_name=None, timeout_seconds=20):
    encoded, _ = _named_runner_command(
        "screenshot",
        daemon_name=daemon_name,
        timeout_seconds=timeout_seconds,
        full=full,
    )
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
    payload = {
        "selector": selector,
        "files": list(files),
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
