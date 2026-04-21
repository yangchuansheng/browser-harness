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
- keep the implementation declarative until the runtime boundary is more proven

Current commands:

```bash
cd rust
cargo run --quiet --bin bhrun -- manifest
cargo run --quiet --bin bhrun -- sample-config
cargo run --quiet --bin bhrun -- capabilities
```

These commands are not a guest runtime yet. They are only a design scaffold.

## Before Real Guest Execution

Do not start real guest execution until these are true:

- `new_tab(url)` semantics are fixed and tested
- typed helper semantics are stable in live browser validation
- the generated CDP layer design is chosen
- the event model is clear enough to expose safely
- the capability model is stable enough that guests are not forced onto `cdp_raw`
