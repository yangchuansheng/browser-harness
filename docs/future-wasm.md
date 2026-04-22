# Future WASM Direction

Browser Harness already supports Rust/WASM guests. This document captures the
longer-term direction for that layer.

## Goal

Keep the host boundary small and stable:

- `bhd` owns the browser connection and session state
- `bhctl` owns admin and browser lifecycle
- `bhrun` owns guest execution and capability routing
- guests stay dynamic and capability-gated

## Target Shape

```text
WASM guest
  -> bhrun
  -> bh-wasm-host
  -> bhd
  -> CDP / Browser Use
```

Guests should not:

- talk to the daemon socket directly
- provision browsers
- own daemon lifecycle

## API Layers

### Host utilities

These belong in the host boundary:

- `wait`
- `wait_for_event`
- `watch_events`
- `wait_for_request`
- `wait_for_response`
- `http_get`

### Typed compatibility helpers

These keep guest authoring ergonomic:

- tab/session helpers
- `goto`
- `wait_for_load`
- `js`
- input helpers
- viewport, screenshot, PDF, upload, cookies, downloads

### Escape hatch

`cdp_raw` remains useful, but should stay:

- explicit
- capability-gated
- disabled by default for new guests

## Current Scaffold

The current guest path already exists:

- `rust/crates/bh-wasm-host`
- `rust/crates/bh-guest-sdk`
- `rust/guests/`

Useful commands:

```bash
cd rust
cargo run --quiet --bin bhrun -- manifest
cargo run --quiet --bin bhrun -- sample-config
cargo run --quiet --bin bhrun -- capabilities
```

Build a Rust guest to WASM:

```bash
rustup target add --toolchain stable-x86_64-unknown-linux-gnu wasm32-unknown-unknown
cargo +stable build --release --target wasm32-unknown-unknown --manifest-path guests/rust-github-trending/Cargo.toml
```

Run a guest:

```bash
cargo run --quiet --bin bhrun -- run-guest guests/rust-github-trending/target/wasm32-unknown-unknown/release/rust_github_trending_guest.wasm <<'JSON'
{"daemon_name":"default","guest_module":"guests/rust-github-trending/target/wasm32-unknown-unknown/release/rust_github_trending_guest.wasm","granted_operations":["ensure_real_tab","goto","wait_for_load","wait","page_info","js"],"allow_http":false,"allow_raw_cdp":false,"persistent_guest_state":true}
JSON
```

## Near-Term Direction

The useful WASM work now is:

- expanding guest coverage where real domain skills justify it
- keeping the host ABI small
- moving repeated workflow logic into guests only when the host operations are already stable
