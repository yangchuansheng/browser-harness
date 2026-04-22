# Network Requests

Use network signals when page state is ambiguous: submit flows that do not
navigate, SPA actions that repaint in place, downloads, or forms that fail
silently.

## Preferred Order

1. Use `http_get()` when the data is public and does not depend on the live
   browser page.
2. Use `wait_for_response` when a browser action should trigger one specific
   request or response.
3. Use `watch_events` when you do not yet know the exact URL or when you need
   to see the whole burst of activity around an action.
4. Fall back to `drain_events()` only for quick ad hoc inspection in the Python
   compatibility shell.

## Public HTTP First

If a workflow can be satisfied without browser state, prefer pure HTTP. It is
faster, easier to verify, and avoids DOM ambiguity entirely.

```python
import json

data = json.loads(http_get(
    "https://backend.metacritic.com/games/metacritic/the-last-of-us/web"
    "?componentName=product&componentType=Product"
    "&apiKey=1MOZgmNFxvmljaQR1X9KAij9Mo4xAY3u"
))
print(data["data"]["item"]["title"])   # "The Last of Us"
```

Use this path for APIs, SSR payloads such as Walmart `__NEXT_DATA__`, or other
public endpoints where the browser is not part of the real work.

## Exact Browser Waits

When the page matters, wait on the network response instead of guessing from
DOM changes.

Pattern:

1. Get the current session id.
2. Start the wait before the click / submit / navigation.
3. Trigger the browser action.
4. Assert `matched`, `session_id`, URL, and optional HTTP status.
5. Only then inspect `page_info()` or DOM state.

The current Rust runner path already supports this:

- `bhrun current-session`
- `bhrun wait-for-response`
- `bhrun watch-events`
- `bh_guest_sdk::wait_for_response(...)` for Rust/Wasm guests

The repository acceptance script that proves the full two-process pattern is:

- `scripts/bhrun_response_smoke.py`

Use local mode for reliable verification:

```bash
BU_BROWSER_MODE=local BU_DAEMON_IMPL=rust python3 scripts/bhrun_response_smoke.py
```

## Watching The Whole Burst

Use `watch_events` when you need to discover what a flow actually does before
you hard-code an exact response wait.

Useful events to watch first:

- `Page.frameStartedNavigating`
- `Network.requestWillBeSent`
- `Network.responseReceived`
- `Page.loadEventFired`

Scope the watch to the active `session_id` whenever possible so another tab or
iframe does not satisfy the watch accidentally.

The repository smoke for this path is:

- `scripts/bhrun_watch_events_smoke.py`

Local verification:

```bash
BU_BROWSER_MODE=local BU_DAEMON_IMPL=rust python3 scripts/bhrun_watch_events_smoke.py
```

## Python Shell Fallback

The Python compatibility shell does not yet expose first-class wrappers for
`wait_for_response` or `watch_events`. When you stay in `helpers.py` land,
fallback to buffered inspection:

```python
drain_events()          # clear old noise first
click(420, 610)
wait(1.0)

responses = [
    e for e in drain_events()
    if e.get("method") == "Network.responseReceived"
]
for event in responses:
    response = event.get("params", {}).get("response", {})
    print(event.get("session_id"), response.get("status"), response.get("url"))
```

This is good for discovery, but it is weaker than a runner-owned blocking wait
because the buffer is destructive and you can miss short-lived events if you
start looking too late.

## Practical Rules

- Start the wait before the action. Starting after the click is how you miss
  the event and end up reading stale page state.
- Scope to the current session on multi-tab flows.
- Prefer network truth over DOM heuristics for downloads, saves, and SPA
  submits.
- Use `page_info()` after the network confirms success, not instead of the
  network.
- Keep Browser Use remote verification best-effort only; use local browser mode
  as the acceptance path for site-shaped flows.
