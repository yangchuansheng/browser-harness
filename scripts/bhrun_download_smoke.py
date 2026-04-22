#!/usr/bin/env python3
"""Run a live end-to-end smoke test for `bhrun configure-downloads` / `wait-for-download`.

Optional:
  BU_NAME                   defaults to "bhrun-download-smoke"
  BU_BROWSER_MODE           defaults to "local"; "remote" is best-effort only
  BU_DAEMON_IMPL            defaults to "rust"
  BU_REMOTE_TIMEOUT_MINUTES defaults to "1"
  BU_LOCAL_DAEMON_WAIT_SECONDS defaults to "15"
  BU_RUST_RUNNER_BIN        override the bhrun binary path
"""

import json
import os
import subprocess
import sys
import tempfile
import time
import uuid
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO))
os.environ.setdefault("BU_NAME", "bhrun-download-smoke")

from admin import _browser_use, ensure_daemon, restart_daemon, start_remote_daemon  # noqa: E402
from scripts._runner_cli import (  # noqa: E402
    configure_downloads,
    drain_events,
    goto,
    js,
    page_info,
    wait_for_load,
)

TARGET_URL = "https://example.com/?via=bhrun-download-smoke"


def poll_browser_status(browser_id, attempts=10, delay=1.0):
    for _ in range(attempts):
        listing = _browser_use("/browsers?pageSize=20&pageNumber=1", "GET")
        item = next((item for item in listing.get("items", []) if item.get("id") == browser_id), None)
        status = item.get("status") if item else "missing"
        if status != "active":
            return status
        time.sleep(delay)
    return status


def runner_process_spec():
    if custom := os.environ.get("BU_RUST_RUNNER_BIN"):
        return [custom], str(REPO)
    return ["cargo", "run", "--quiet", "--bin", "bhrun", "--"], str(REPO / "rust")


def start_runner_command(subcommand, payload):
    cmd, cwd = runner_process_spec()
    proc = subprocess.Popen(
        cmd + [subcommand],
        cwd=cwd,
        env=os.environ.copy(),
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    assert proc.stdin is not None
    proc.stdin.write(json.dumps(payload))
    proc.stdin.close()
    return proc, payload


def finish_runner_command(proc, timeout_seconds):
    try:
        proc.wait(timeout=timeout_seconds)
    except subprocess.TimeoutExpired:
        proc.kill()
        raise RuntimeError("bhrun command timed out")

    stdout = proc.stdout.read() if proc.stdout is not None else ""
    stderr = proc.stderr.read() if proc.stderr is not None else ""
    if proc.returncode != 0:
        raise RuntimeError((stderr or stdout or f"bhrun exited {proc.returncode}").strip())
    if not stdout.strip():
        raise RuntimeError("bhrun returned empty stdout")
    return json.loads(stdout)


def wait_for_downloaded_file(path, timeout_seconds=10.0):
    deadline = time.time() + timeout_seconds
    partial_path = Path(f"{path}.crdownload")
    while time.time() < deadline:
        if path.exists() and not partial_path.exists():
            return path
        time.sleep(0.2)
    raise RuntimeError(f"downloaded file did not appear at {path}")


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
        result["page_before_download"] = page_info(daemon_name=name)
        drain_events(daemon_name=name)

        with tempfile.TemporaryDirectory(prefix="bhrun-download-smoke-") as tmpdir:
            download_dir = Path(tmpdir)
            filename = f"bhrun-download-{uuid.uuid4().hex[:10]}.txt"
            file_path = download_dir / filename
            file_text = f"bhrun download smoke {uuid.uuid4().hex}"

            configure_downloads(str(download_dir), daemon_name=name)
            result["download_dir"] = str(download_dir)
            result["filename"] = filename

            wait_proc, wait_payload = start_runner_command(
                "wait-for-download",
                {
                    "daemon_name": name,
                    "filename": filename,
                    "timeout_ms": 5000,
                    "poll_interval_ms": 100,
                },
            )
            result["wait_request"] = wait_payload
            time.sleep(0.4)
            result["trigger_result"] = js(
                f"""
(() => {{
  const text = {json.dumps(file_text)};
  const blob = new Blob([text], {{type: 'text/plain'}});
  const href = URL.createObjectURL(blob);
  const link = document.createElement('a');
  link.href = href;
  link.download = {json.dumps(filename)};
  document.body.appendChild(link);
  link.click();
  setTimeout(() => {{
    URL.revokeObjectURL(href);
    link.remove();
  }}, 250);
  return {{href, filename: link.download, textLength: text.length}};
}})()
""",
                daemon_name=name,
                timeout_seconds=20,
            )

            wait_result = finish_runner_command(wait_proc, timeout_seconds=15)
            result["wait_result"] = wait_result
            event = wait_result.get("event") or {}
            params = event.get("params") or {}
            if not wait_result.get("matched"):
                raise RuntimeError("wait-for-download returned matched=false")
            if event.get("method") != "Browser.downloadWillBegin":
                raise RuntimeError(f"unexpected download event method: {event.get('method')!r}")
            if params.get("suggestedFilename") != filename:
                raise RuntimeError(
                    "download event filename mismatch: "
                    f"{params.get('suggestedFilename')!r} vs {filename!r}"
                )

            if browser_mode == "local":
                downloaded = wait_for_downloaded_file(file_path)
                downloaded_text = downloaded.read_text()
                result["downloaded_file"] = str(downloaded)
                result["downloaded_bytes"] = downloaded.stat().st_size
                result["downloaded_text"] = downloaded_text
                if downloaded_text != file_text:
                    raise RuntimeError("downloaded file content did not match the blob payload")
            else:
                result["download_verification"] = "event_only"

        result["page_after_download"] = page_info(daemon_name=name)
        if result["page_after_download"].get("url") != TARGET_URL:
            raise RuntimeError("page URL changed during download smoke")
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
