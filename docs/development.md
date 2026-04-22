# Development

This document is for working on the current Browser Harness codebase.

## Workspace Layout

- `rust/bins/browser-harness-cli`: top-level CLI facade
- `rust/bins/bhctl`: admin/control plane binary
- `rust/bins/bhrun`: typed runner and guest executor
- `rust/bins/bhd`: daemon/runtime core
- `rust/bins/bhsmoke`: smoke verification binary
- `rust/crates/*`: shared libraries for protocol, discovery, remote control, guest hosting, and SDKs
- `rust/guests/*`: WAT and Rust-to-WASM guest samples

## Common Commands

Build the workspace:

```bash
cargo build --workspace --manifest-path rust/Cargo.toml
```

Run tests:

```bash
cargo test --workspace --manifest-path rust/Cargo.toml
```

Install the Rust CLI binaries into `$CARGO_HOME/bin`:

```bash
cargo run --quiet --manifest-path rust/Cargo.toml --bin browser-harness -- install
```

Verify the installed package surface:

```bash
browser-harness verify-install
```

## Local Browser Verification

Local browser smokes are the main acceptance path.

If Chrome discovery is already configured:

```bash
browser-harness ensure-daemon
rust/target/debug/bhsmoke guest-run
```

If you need to pin an exact local websocket:

```bash
export BU_BROWSER_MODE=local
export BU_DAEMON_IMPL=rust
export BU_CDP_WS=ws://localhost:<port>/devtools/browser/<id>
rust/target/debug/bhsmoke actions
rust/target/debug/bhsmoke tabs
rust/target/debug/bhsmoke guest-run
rust/target/debug/bhsmoke set-viewport
rust/target/debug/bhsmoke upload-file
rust/target/debug/bhsmoke github-trending-guest
BU_2048_TARGET=256 rust/target/debug/bhsmoke 2048-guest
```

## Remote Verification

Remote verification is supported but should be treated as best-effort:

```bash
export BROWSER_USE_API_KEY=...
rust/target/debug/bhsmoke remote
rust/target/debug/bhsmoke wait-for-load-event
rust/target/debug/bhsmoke wait-for-response
```

Remote failures can come from Browser Use quota/billing or from live-site
behavior, so local verification remains the main release gate.

## Formatting And Hygiene

Format Rust code with:

```bash
cargo fmt --all --manifest-path rust/Cargo.toml
```

Scan for secrets or local path leaks with:

```bash
./scripts/scan_sensitive.sh
```

## Python

The repository no longer contains an active Python runtime layer.

If you intentionally want to drive the Rust CLI from Python, use:

- `docs/python-integration.md`

That document shows the supported `subprocess` wrapper pattern.
