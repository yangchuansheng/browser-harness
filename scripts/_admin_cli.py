import json
import os
import subprocess
import time
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]


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


def _daemon_name(name=None):
    return name or os.environ.get("BU_NAME", "default")


def admin_process_spec():
    if custom := os.environ.get("BU_BROWSER_HARNESS_BIN"):
        return [custom], str(REPO)
    if custom := os.environ.get("BU_RUST_ADMIN_BIN"):
        return [custom], str(REPO)
    return ["cargo", "run", "--quiet", "--bin", "browser-harness", "--"], str(REPO / "rust")


def run_admin_command(subcommand, payload=None, extra_args=None, timeout_seconds=60):
    cmd, cwd = admin_process_spec()
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
        raise RuntimeError((stderr or stdout or f"admin command exited {proc.returncode}").strip())
    if not stdout.strip():
        raise RuntimeError("admin command returned empty stdout")
    return json.loads(stdout)


def daemon_alive(name=None, timeout_seconds=10):
    result = run_admin_command(
        "daemon-alive",
        extra_args=[_daemon_name(name)],
        timeout_seconds=timeout_seconds,
    )
    return bool(result.get("alive"))


def ensure_daemon(wait=60.0, name=None, env=None, timeout_seconds=None):
    return run_admin_command(
        "ensure-daemon",
        {
            "wait": wait,
            "name": _daemon_name(name),
            "env": env or {},
        },
        timeout_seconds=timeout_seconds or max(60, int(wait) + 10),
    )


def restart_daemon(name=None, timeout_seconds=20):
    return run_admin_command(
        "restart-daemon",
        extra_args=[_daemon_name(name)],
        timeout_seconds=timeout_seconds,
    )


def create_browser(timeout_seconds=60, **payload):
    return run_admin_command(
        "create-browser",
        payload,
        timeout_seconds=timeout_seconds,
    )


def stop_browser(browser_id, timeout_seconds=30):
    return run_admin_command(
        "stop-browser",
        extra_args=[browser_id],
        timeout_seconds=timeout_seconds,
    )


def list_browsers(page_size=20, page_number=1, timeout_seconds=30):
    return run_admin_command(
        "list-browsers",
        {
            "pageSize": page_size,
            "pageNumber": page_number,
        },
        timeout_seconds=timeout_seconds,
    )


def poll_browser_status(browser_id, attempts=10, delay=1.0, page_size=20):
    status = "missing"
    for _ in range(attempts):
        listing = list_browsers(page_size=page_size)
        item = next((item for item in listing.get("items", []) if item.get("id") == browser_id), None)
        status = item.get("status") if item else "missing"
        if status != "active":
            return status
        time.sleep(delay)
    return status


def start_remote_daemon(name="remote", profile_name=None, timeout_seconds=60, **create_kwargs):
    daemon_name = _daemon_name(name)
    if daemon_alive(daemon_name, timeout_seconds=10):
        raise RuntimeError(f"daemon {daemon_name!r} already alive -- restart_daemon({daemon_name!r}) first")

    payload = dict(create_kwargs)
    if profile_name is not None:
        payload["profileName"] = profile_name
    browser = create_browser(timeout_seconds=timeout_seconds, **payload)
    ensure_daemon(
        name=daemon_name,
        env={"BU_CDP_WS": browser["cdpWsUrl"], "BU_BROWSER_ID": browser["id"]},
        timeout_seconds=timeout_seconds,
    )
    return browser
