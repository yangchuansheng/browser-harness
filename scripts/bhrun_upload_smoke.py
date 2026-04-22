#!/usr/bin/env python3
"""Run a live end-to-end smoke test for `bhrun upload-file`.

Optional:
  BU_NAME                   defaults to "bhrun-upload-smoke"
  BU_BROWSER_MODE           defaults to "local"
  BU_DAEMON_IMPL            defaults to "rust"
  BU_LOCAL_DAEMON_WAIT_SECONDS defaults to "15"
"""

import json
import os
import sys
import tempfile
import time
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO))
os.environ.setdefault("BU_NAME", "bhrun-upload-smoke")

from admin import ensure_daemon, restart_daemon  # noqa: E402
from scripts._runner_cli import goto, js, page_info, set_viewport, upload_file, wait, wait_for_load  # noqa: E402


def main():
    browser_mode = os.environ.get("BU_BROWSER_MODE", "local").strip().lower() or "local"
    if browser_mode != "local":
        raise SystemExit("bhrun_upload_smoke.py currently supports only BU_BROWSER_MODE=local")

    os.environ.setdefault("BU_DAEMON_IMPL", "rust")
    name = os.environ["BU_NAME"]
    local_wait = float(os.environ.get("BU_LOCAL_DAEMON_WAIT_SECONDS", "15"))

    result = {
        "name": name,
        "daemon_impl": os.environ["BU_DAEMON_IMPL"],
        "browser_mode": browser_mode,
    }
    try:
        ensure_daemon(name=name, wait=local_wait)
        result["local_attach"] = "DevToolsActivePort"

        goto("about:blank", daemon_name=name)
        result["loaded"] = wait_for_load(daemon_name=name)
        set_viewport(900, 700, daemon_name=name)
        wait(0.2)
        result["page_before_upload"] = page_info(daemon_name=name)

        js(
            """
(() => {
  document.body.innerHTML = `
    <style>body{font-family:monospace;padding:32px;background:#f5f0e8}</style>
    <label for="upload">Upload fixture</label>
    <input id="upload" type="file" multiple />
    <pre id="state"></pre>
  `;
  window.__uploadState = {ready: false, names: [], texts: []};
  const input = document.getElementById('upload');
  const state = document.getElementById('state');
  input.addEventListener('change', async () => {
    const files = Array.from(input.files || []);
    window.__uploadState = {
      ready: true,
      names: files.map(file => file.name),
      sizes: files.map(file => file.size),
      texts: await Promise.all(files.map(file => file.text()))
    };
    state.textContent = JSON.stringify(window.__uploadState);
  });
  return true;
})()
""",
            daemon_name=name,
        )

        with tempfile.TemporaryDirectory(prefix="bhrun-upload-smoke-") as tmpdir:
            file_path = Path(tmpdir) / "upload-fixture.txt"
            file_text = "bhrun upload smoke payload"
            file_path.write_text(file_text)
            result["upload_file"] = str(file_path)

            upload_file("#upload", [str(file_path)], daemon_name=name)
            wait(0.3)
            upload_state = js("window.__uploadState", daemon_name=name)
            result["upload_state"] = upload_state

            if not upload_state.get("ready"):
                raise RuntimeError("file input change handler did not run")
            if upload_state.get("names") != [file_path.name]:
                raise RuntimeError(f"unexpected uploaded file names: {upload_state.get('names')!r}")
            if upload_state.get("texts") != [file_text]:
                raise RuntimeError(f"unexpected uploaded file text: {upload_state.get('texts')!r}")

        result["page_after_upload"] = page_info(daemon_name=name)
        if result["page_after_upload"].get("url") != "about:blank":
            raise RuntimeError("page URL changed during upload smoke")
    finally:
        restart_daemon(name)
        time.sleep(1)
        log_path = Path(f"/tmp/bu-{name}.log")
        if log_path.exists():
            lines = log_path.read_text().strip().splitlines()
            result["log_tail"] = lines[-8:]

    print(json.dumps(result))


if __name__ == "__main__":
    main()
