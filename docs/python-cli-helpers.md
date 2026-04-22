# Python Via The Rust CLI

The active source tree no longer keeps repo-local Python shims such as
`runner_cli.py` or `admin_cli.py`.

If you intentionally want to keep extraction logic in Python, call the Rust
CLI directly through `subprocess`:

```python
import json
import subprocess


def bh_call(command, payload=None):
    proc = subprocess.run(
        ["browser-harness", command],
        input="" if payload is None else json.dumps(payload),
        text=True,
        capture_output=True,
        check=True,
    )
    stdout = proc.stdout.strip()
    return None if not stdout else json.loads(stdout)
```

## HTTP Helper

Use this when the workflow is pure HTTP and does not depend on a live browser:

```python
def http_get(url, headers=None, timeout=30.0):
    payload = {"url": url, "timeout": timeout}
    if headers:
        payload["headers"] = headers
    return bh_call("http-get", payload)
```

## Browser Tab Helpers

Use these when the example needs a real attached browser tab:

```python
def goto(url, daemon_name="default"):
    return bh_call("goto", {"daemon_name": daemon_name, "url": url})


def new_tab(url="about:blank", daemon_name="default"):
    return bh_call("new-tab", {"daemon_name": daemon_name, "url": url})["target_id"]


def wait_for_load(timeout=15.0, daemon_name="default"):
    return bool(
        bh_call(
            "wait-for-load",
            {"daemon_name": daemon_name, "timeout": timeout},
        )
    )


def wait(seconds=1.0):
    return bh_call("wait", {"duration_ms": max(0, int(seconds * 1000))})


def js(expression, target_id=None, daemon_name="default"):
    payload = {"daemon_name": daemon_name, "expression": expression}
    if target_id is not None:
        payload["target_id"] = target_id
    return bh_call("js", payload)


def page_info(daemon_name="default"):
    return bh_call("page-info", {"daemon_name": daemon_name})


def scroll(x, y, dy=-300, dx=0, daemon_name="default"):
    return bh_call(
        "scroll",
        {
            "daemon_name": daemon_name,
            "x": x,
            "y": y,
            "dy": dy,
            "dx": dx,
        },
    )
```

## Environment

For browser commands, keep using the same environment variables as the rest of
the Rust CLI surface:

- `BU_BROWSER_MODE=local` for local Chrome / Edge attach
- `BU_DAEMON_IMPL=rust` to force the Rust daemon
- `BU_CDP_WS=ws://...` when you want to pin the exact browser websocket

The helpers above are intentionally thin. If you need a command not listed
here, add another small wrapper over `bh_call(...)` instead of reviving the old
repo-local Python shims.
