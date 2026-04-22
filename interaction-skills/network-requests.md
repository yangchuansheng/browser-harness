# Network Requests

Use network signals when page state is ambiguous: submit flows that do not
navigate, SPA actions that repaint in place, downloads, or forms that fail
silently.

## Preferred Order

1. Use `http_get()` when the data is public and does not depend on the live
   browser page.
2. Use `wait_for_request` when you need proof that the page actually issued a
   request, even if the response may fail or does not matter yet.
3. Use `wait_for_response` when backend success matters and you need to assert
   status or the final response URL.
4. Use `watch_events` when you do not yet know the exact URL or when you need
   to see the whole burst of activity around an action.
5. Treat raw event draining as a discovery fallback, not the normal path.

## Public HTTP First

If a workflow can be satisfied without browser state, prefer pure HTTP. It is
faster, easier to verify, and avoids DOM ambiguity entirely.

```bash
browser-harness http-get <<'JSON' | jq -r . | jq -r '.data.item.title'
{"url":"https://backend.metacritic.com/games/metacritic/the-last-of-us/web?componentName=product&componentType=Product&apiKey=1MOZgmNFxvmljaQR1X9KAij9Mo4xAY3u","timeout":20.0}
JSON
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
- `browser-harness current-session`
- `bhrun wait-for-event`
- `bhrun wait-for-request`
- `bhrun wait-for-response`
- `bhrun watch-events`
- `bhrun wait-for-console`
- `browser-harness wait-for-request`
- `browser-harness wait-for-response`
- `browser-harness watch-events`
- `bh_guest_sdk::wait_for_event(...)`
- `bh_guest_sdk::watch_events(...)`
- `bh_guest_sdk::wait_for_request(...)` for Rust/Wasm guests
- `bh_guest_sdk::wait_for_response(...)` for Rust/Wasm guests
- `bh_guest_sdk::wait_for_console(...)` for Rust/Wasm guests

The repository acceptance runner for the full two-process pattern is:

- `rust/bins/bhsmoke` with the `wait-for-request` and `wait-for-response`
  scenarios

Use local mode for reliable verification:

```bash
BU_BROWSER_MODE=local BU_DAEMON_IMPL=rust cargo run --quiet --manifest-path rust/Cargo.toml --bin bhsmoke -- wait-for-request
BU_BROWSER_MODE=local BU_DAEMON_IMPL=rust cargo run --quiet --manifest-path rust/Cargo.toml --bin bhsmoke -- wait-for-response
```

Use request waits when outbound intent is the real success signal:

- fire-and-forget analytics or beacon calls
- form submissions where the response is opaque or cross-origin
- proving that a page action emitted one exact request before you tighten to
  response assertions

Use response waits when server success matters:

- save/submit flows
- searches
- data refreshes
- navigation or API calls where status must be checked

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

- `rust/bins/bhsmoke` with the `wait-for-request`, `watch-events`, and
  `event-waits-guest` scenarios

Local verification:

```bash
BU_BROWSER_MODE=local BU_DAEMON_IMPL=rust cargo run --quiet --manifest-path rust/Cargo.toml --bin bhsmoke -- wait-for-request
BU_BROWSER_MODE=local BU_DAEMON_IMPL=rust cargo run --quiet --manifest-path rust/Cargo.toml --bin bhsmoke -- watch-events
BU_BROWSER_MODE=local BU_DAEMON_IMPL=rust cargo run --quiet --manifest-path rust/Cargo.toml --bin bhsmoke -- event-waits-guest
```

## CLI Fallback

If you are still discovering the exact network pattern, you can fall back to
buffered event draining through the Rust CLI:

```bash
browser-harness drain-events <<'JSON'
{"daemon_name":"default"}
JSON
# trigger the browser action here through browser-harness / bhrun / another script
browser-harness wait <<'JSON'
{"duration_ms":1000}
JSON
browser-harness drain-events <<'JSON' | jq '.[] | select(.method=="Network.responseReceived") | {session_id, status: .params.response.status, url: .params.response.url}'
{"daemon_name":"default"}
JSON
```

This is weaker than a runner-owned blocking wait because the buffer is
destructive and you can miss short-lived events if you start looking too late.
The archived `helpers.py` shell had the same limitation and is no longer the
preferred teaching surface.

## Practical Rules

- Start the wait before the action. Starting after the click is how you miss
  the event and end up reading stale page state.
- Use `wait_for_request` when you are proving that the browser sent something
  at all; use `wait_for_response` when you are proving the backend answered.
- Scope to the current session on multi-tab flows.
- Prefer network truth over DOM heuristics for downloads, saves, and SPA
  submits.
- Use `page_info()` after the network confirms success, not instead of the
  network.
- Keep Browser Use remote verification best-effort only; use local browser mode
  as the acceptance path for site-shaped flows.
