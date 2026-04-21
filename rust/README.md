# Rust Workspace

This workspace is the starting point for the Browser Harness rewrite.

The near-term goal is a Rust daemon/core that preserves the current Python
workflow. The long-term goal is a Rust host with a WASM guest layer.

Current status:

- crate layout only
- Rust daemon connects to local or remote CDP and serves the existing Unix socket contract
- first typed helper operations are implemented in the Rust daemon: page info, tab listing/current tab, tab switching, new-tab creation, real-tab recovery, iframe lookup, load waiting, JS evaluation, goto, screenshot capture, low-level input primitives, DOM key dispatch, and file upload
- remote-browser shutdown parity is implemented in the Rust daemon
- local regression tests cover protocol, discovery, remote stop requests, daemon buffer behavior, and Python Rust-mode compatibility paths
- long-term WASM design scaffolding exists via `bh-wasm-host`, `bhrun`, and [docs/wasm-runner-design.md](/home/allosaurus/Workspace/browser-harness/docs/wasm-runner-design.md)

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
cargo run --quiet --bin bhrun -- current-session <<'JSON'
{"daemon_name":"default"}
JSON
cargo run --quiet --bin bhrun -- wait-for-event <<'JSON'
{"daemon_name":"default","filter":{"method":"Page.loadEventFired"}}
JSON
cargo run --quiet --bin bhrun -- wait-for-load-event <<'JSON'
{"daemon_name":"default","session_id":"<current-session-id>"}
JSON
cargo run --quiet --bin bhrun -- wait-for-response <<'JSON'
{"daemon_name":"default","session_id":"<current-session-id>","url":"https://example.com/api","status":200}
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

Live `bhrun wait-for-response` smoke:

```bash
BROWSER_USE_API_KEY=... python3 scripts/bhrun_response_smoke.py
```
