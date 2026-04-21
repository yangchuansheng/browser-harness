# WASM Runner Design

## Purpose

This document captures the long-term Rust + WASM target without changing the
current short-term compatibility goal.

The intent is:

- keep `bhd` as the browser/session owner
- keep `bhctl` as the admin/control boundary
- add `bhrun` as a persistent guest runner
- move dynamic task logic into WASM only after the host boundary is proven stable

## Target Shape

```text
WASM guest module
  -> bhrun
  -> bh-wasm-host
  -> bhd socket
  -> CDP / Browser Use
```

Important constraints:

- guests do not talk to the daemon socket directly
- guests do not own browser lifecycle or recovery
- the runner must preserve guest state across calls
- host failures should not kill the daemon session, and guest failures should not kill the daemon

## API Layers

The long-term guest boundary should be protocol-first, not helper-first.

### 1. Generated protocol families

These are the long-term foundation:

- `cdp.browser_protocol`
- `cdp.js_protocol`

They should be generated from vendored Chrome protocol schemas, not hand-written.

Why:

- reduces drift with Chrome
- makes the host surface less ad hoc
- gives the WASM layer a stable low-level substrate

### 2. Host utilities

These are small operations that do not belong in generated CDP:

- `wait`
- `wait_for_event`
- `watch_events`
- `http_get`

Other utilities can exist, but they should stay narrow and clearly host-owned.

### 3. Compatibility helpers

These preserve the current product ergonomics while the project transitions:

- `page_info`
- `list_tabs`
- `current_tab`
- `new_tab`
- `switch_tab`
- `ensure_real_tab`
- `iframe_target`
- `goto`
- `wait_for_load`
- `js`
- `click`
- `type_text`
- `press_key`
- `dispatch_key`
- `scroll`
- `upload_file`

These helpers are useful, but they should be layered above generated protocol
access instead of defining the entire long-term ABI.

### 4. Escape hatch

`cdp_raw` should remain available, but:

- it should be explicit
- it should be capability-gated
- it should be disabled by default for new guests

## Runner Model

`bhrun` should be a persistent runner rather than a one-shot executor.

The runner should eventually own:

- guest module loading
- guest state persistence between invocations
- capability granting
- event waiting/filtering utilities
- host-call routing into `bhd`

The runner should not own:

- browser discovery
- browser lifecycle
- daemon lifecycle
- remote-browser provisioning

Those stay in `bhd` / `bhctl`.

## Immediate Scaffold

Current scaffold goals:

- define a manifest for protocol families and guest-exposed operations
- define a sample runner configuration
- define the first guest authoring SDK layer above the raw `bh.call_json` import
- keep the implementation small until the runtime boundary is more proven

Current commands:

```bash
cd rust
cargo run --quiet --bin bhrun -- manifest
cargo run --quiet --bin bhrun -- sample-config
cargo run --quiet --bin bhrun -- capabilities
cargo run --quiet --bin bhrun -- run-guest guests/navigate_and_read.wat <<'JSON'
{"daemon_name":"default","guest_module":"guests/navigate_and_read.wat","granted_operations":["goto","wait_for_load_event","page_info","js"],"allow_http":false,"allow_raw_cdp":false,"persistent_guest_state":true}
JSON
rustup target add --toolchain stable-x86_64-unknown-linux-gnu wasm32-unknown-unknown
cargo +stable build --release --target wasm32-unknown-unknown --manifest-path guests/rust-navigate-and-read/Cargo.toml
cargo run --quiet --bin bhrun -- run-guest guests/rust-navigate-and-read/target/wasm32-unknown-unknown/release/rust_navigate_and_read_guest.wasm <<'JSON'
{"daemon_name":"default","guest_module":"guests/rust-navigate-and-read/target/wasm32-unknown-unknown/release/rust_navigate_and_read_guest.wasm","granted_operations":["goto","wait_for_load_event","page_info","js"],"allow_http":false,"allow_raw_cdp":false,"persistent_guest_state":true}
JSON
cargo run --quiet --bin bhrun -- wait <<'JSON'
{"duration_ms":1}
JSON
cat <<'NDJSON' | cargo run --quiet --bin bhrun -- serve-guest guests/persistent_counter.wat
{"command":"start","config":{"daemon_name":"default","guest_module":"guests/persistent_counter.wat","granted_operations":["wait"],"allow_http":false,"allow_raw_cdp":false,"persistent_guest_state":true}}
{"command":"run"}
{"command":"run"}
{"command":"stop"}
NDJSON
cargo run --quiet --bin bhrun -- current-tab <<'JSON'
{"daemon_name":"default"}
JSON
cargo run --quiet --bin bhrun -- list-tabs <<'JSON'
{"daemon_name":"default","include_internal":true}
JSON
cargo run --quiet --bin bhrun -- new-tab <<'JSON'
{"daemon_name":"default","url":"https://example.com"}
JSON
cargo run --quiet --bin bhrun -- switch-tab <<'JSON'
{"daemon_name":"default","target_id":"<target-id>"}
JSON
cargo run --quiet --bin bhrun -- page-info <<'JSON'
{"daemon_name":"default"}
JSON
cargo run --quiet --bin bhrun -- goto <<'JSON'
{"daemon_name":"default","url":"https://example.com"}
JSON
cargo run --quiet --bin bhrun -- js <<'JSON'
{"daemon_name":"default","expression":"location.href"}
JSON
cargo run --quiet --bin bhrun -- current-session <<'JSON'
{"daemon_name":"default"}
JSON
cargo run --quiet --bin bhrun -- wait-for-event <<'JSON'
{"daemon_name":"default","filter":{"method":"Page.loadEventFired"}}
JSON
cargo run --quiet --bin bhrun -- watch-events <<'JSON'
{"daemon_name":"default","filter":{"session_id":"<current-session-id>"},"timeout_ms":2000,"max_events":10}
JSON
cargo run --quiet --bin bhrun -- wait-for-load-event <<'JSON'
{"daemon_name":"default","session_id":"<current-session-id>"}
JSON
cargo run --quiet --bin bhrun -- wait-for-response <<'JSON'
{"daemon_name":"default","session_id":"<current-session-id>","url":"https://example.com/api","status":200}
JSON
cargo run --quiet --bin bhrun -- wait-for-console <<'JSON'
{"daemon_name":"default","session_id":"<current-session-id>","type":"log","text":"ready"}
JSON
cargo run --quiet --bin bhrun -- wait-for-dialog <<'JSON'
{"daemon_name":"default","session_id":"<current-session-id>","type":"alert","message":"ready"}
JSON
```

These commands are not a full guest runtime yet, but the first preview guest
execution slice is live.
`run-guest` currently loads a core Wasm module, exposes a single generic
`bh.call_json` import, enforces `RunnerConfig.granted_operations`, and returns a
call trace for the guest's host interactions.
`bh-guest-sdk` is the first Rust guest authoring layer above that import, and
`guests/rust-navigate-and-read` is the first compiled Rust guest sample using it.
`serve-guest` is the first persistent runner preview. It keeps one Wasm
instance alive, accepts line-delimited control messages, and reuses the same
guest state across repeated `run` invocations.
`guests/rust-persistent-browser-state` is the first compiled Rust guest that
depends on that persistence model across repeated `serve-guest` runs.
That boundary now has both a browser-free persistence smoke and a live
browser-backed persistence smoke.
`current-tab`, `list-tabs`, `new-tab`, and `switch-tab` are the first live
runner-owned target control helpers, giving guests direct tab/session selection
without reaching around the runner boundary.
`page-info`, `goto`, and `js` are the first live runner-owned action helpers,
bridging into the daemon's typed compatibility surface without going through the
Python shell.
`wait` is the first runner-local utility that does not require browser I/O,
which makes browser-free guest/runtime persistence checks possible.
`wait-for-event` is the first live Phase 2 runner primitive.
`watch-events` is the first generic streaming primitive layered on the same daemon event buffer.
`wait-for-load-event` is the first helper layered directly on top of it.
`current-session` is the runner-side introspection helper for session-scoped waits.
`wait-for-response` is the first network helper layered on the same event contract.
`wait-for-console` is the first console/debugging helper layered on the same event contract.
`wait-for-dialog` is the first dialog helper layered on the same event contract.

## Current Event Contract

The first real runner-owned primitive is event waiting over the daemon's
existing `drain_events` buffer.

Current request shape:

```json
{
  "daemon_name": "default",
  "filter": {
    "method": "Network.responseReceived",
    "session_id": "session-1",
    "params_subset": {
      "response": {
        "status": 200
      }
    }
  },
  "timeout_ms": 15000,
  "poll_interval_ms": 200
}
```

Current `watch-events` output is NDJSON:

```json
{"kind":"event","event":{"method":"Page.frameStartedNavigating","session_id":"session-1"},"index":1,"elapsed_ms":87}
{"kind":"event","event":{"method":"Page.loadEventFired","session_id":"session-1"},"index":2,"elapsed_ms":214}
{"kind":"end","matched_events":2,"polls":4,"elapsed_ms":401,"timed_out":true,"reached_max_events":false}
```

Current response shape:

```json
{
  "matched": true,
  "event": {
    "method": "Network.responseReceived",
    "params": {
      "response": {
        "status": 200
      }
    },
    "session_id": "session-1"
  },
  "polls": 3,
  "elapsed_ms": 421
}
```

Matching rules:

- `method` is exact string equality
- `session_id` is exact string equality
- `params_subset` is a recursive object subset match against the event's
  top-level `params`

Session guidance:

- use `current-session` to fetch the runner-visible active session id before
  issuing a session-scoped wait
- omit `session_id` only when any matching event is acceptable
- pass `session_id` for multi-tab or iframe-sensitive waits so the runner does
  not consume another target's event by accident
- `watch-events` is the runner's generic streaming primitive and should be
  preferred when a guest needs to observe more than one matching event before
  making a decision
- `wait-for-load-event` is the runner's convenience wrapper for the common
  `Page.loadEventFired` case and should be preferred over a handwritten filter
  when you already know the session you care about
- `wait-for-response` is the runner's convenience wrapper for
  `Network.responseReceived` and should be preferred when you know the exact URL
  and optional status you want to observe
- `wait-for-console` is the runner's convenience wrapper for
  browser console events and should be preferred when you know the session,
  optional console `type`, and exact message text you want to observe; today it
  matches `Console.messageAdded` live and also tolerates
  `Runtime.consoleAPICalled` where that is exposed
- `wait-for-dialog` is the runner's convenience wrapper for
  `Page.javascriptDialogOpening` and should be preferred when you know the
  session, optional dialog `type`, and exact message text you want to observe
  before dismissing the dialog via CDP

This keeps the first runner contract small while still being expressive enough
for page lifecycle, network, dialog, and console events.

## Before Real Guest Execution

Do not start real guest execution until these are true:

- `new_tab(url)` semantics are fixed and tested
- typed helper semantics are stable in live browser validation
- the generated CDP layer design is chosen
- the event model is clear enough to expose safely
- the capability model is stable enough that guests are not forced onto `cdp_raw`
