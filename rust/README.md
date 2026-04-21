# Rust Workspace

This workspace is the starting point for the Browser Harness rewrite.

The near-term goal is a Rust daemon/core that preserves the current Python
workflow. The long-term goal is a Rust host with a WASM guest layer.

Current status:

- Phase 1 hybrid rewrite is complete: Rust owns the daemon/runtime core and Python remains the compatibility shell
- Rust daemon connects to local or remote CDP and serves the existing Unix socket contract
- first typed helper operations are implemented in the Rust daemon: page info, tab listing/current tab, tab switching, new-tab creation, real-tab recovery, iframe lookup, load waiting, JS evaluation, goto, screenshot capture, low-level input primitives, DOM key dispatch, and file upload
- remote-browser shutdown parity is implemented in the Rust daemon
- local regression tests cover protocol, discovery, remote stop requests, daemon buffer behavior, and Python Rust-mode compatibility paths
- live acceptance coverage includes the GitHub domain skill workflow from `domain-skills/github/scraping.md`
- the first preview guest-execution slice exists via `bh-wasm-host`, `bhrun`, and [docs/wasm-runner-design.md](/home/allosaurus/Workspace/browser-harness/docs/wasm-runner-design.md)
- `bhrun` now has a first persistent guest-runner preview via `serve-guest`, plus the runner-local `wait` utility for browser-free guest verification
- the first Rust guest authoring path now exists via `bh-guest-sdk` and `guests/rust-navigate-and-read`

Compatibility contract:

- [docs/rust-compat-contract.md](/home/allosaurus/Workspace/browser-harness/docs/rust-compat-contract.md)

Quick verification:

```bash
cd rust
cargo test --workspace
```

WASM design scaffold:

```bash
cd rust
cargo run --quiet --bin bhrun -- manifest
cargo run --quiet --bin bhrun -- sample-config
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

Python compatibility tests:

```bash
python3 -m unittest tests/test_rust_mode_contract.py
```

Live remote smoke test:

```bash
BROWSER_USE_API_KEY=... python3 scripts/remote_smoke.py
```

Live `bhrun wait-for-event` smoke:

```bash
BROWSER_USE_API_KEY=... python3 scripts/bhrun_event_smoke.py
```

Live `bhrun watch-events` smoke:

```bash
BROWSER_USE_API_KEY=... python3 scripts/bhrun_watch_events_smoke.py
```

Live `bhrun wait-for-response` smoke:

```bash
BROWSER_USE_API_KEY=... python3 scripts/bhrun_response_smoke.py
```

Live `bhrun wait-for-console` smoke:

```bash
BROWSER_USE_API_KEY=... python3 scripts/bhrun_console_smoke.py
```

Live `bhrun wait-for-dialog` smoke:

```bash
BROWSER_USE_API_KEY=... python3 scripts/bhrun_dialog_smoke.py
```

Live `bhrun` action smoke:

```bash
BROWSER_USE_API_KEY=... python3 scripts/bhrun_actions_smoke.py
```

Live `bhrun` tab/session smoke:

```bash
BROWSER_USE_API_KEY=... python3 scripts/bhrun_tabs_smoke.py
```

Live `bhrun run-guest` smoke:

```bash
BROWSER_USE_API_KEY=... python3 scripts/bhrun_guest_smoke.py
BROWSER_USE_API_KEY=... BU_GUEST_PATH="$PWD/rust/guests/rust-navigate-and-read/target/wasm32-unknown-unknown/release/rust_navigate_and_read_guest.wasm" python3 scripts/bhrun_guest_smoke.py
BROWSER_USE_API_KEY=... BU_GUEST_MODE=serve-guest BU_GUEST_PATH="$PWD/rust/guests/rust-navigate-and-read/target/wasm32-unknown-unknown/release/rust_navigate_and_read_guest.wasm" python3 scripts/bhrun_guest_smoke.py
```

Live `bhrun serve-guest` smoke:

```bash
BROWSER_USE_API_KEY=... python3 scripts/bhrun_persistent_guest_remote_smoke.py
```

Local `bhrun serve-guest` smoke:

```bash
python3 scripts/bhrun_persistent_guest_smoke.py
```

Live GitHub domain-skill acceptance smoke:

```bash
BROWSER_USE_API_KEY=... python3 scripts/domain_skill_github_smoke.py
```
