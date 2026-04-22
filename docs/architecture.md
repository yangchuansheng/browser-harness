# Architecture

Browser Harness is a thin CDP runtime for agents. The active system is the Rust
runtime.

## Main Components

- `browser-harness`: top-level CLI facade
- `bhctl`: browser lifecycle and admin/control plane
- `bhrun`: typed browser operations, waits, and guest execution
- `bhd`: daemon that owns the browser connection and session state
- `bhsmoke`: repo-local smoke runner for verification

## Runtime Flow

```text
Chrome / Browser Use cloud
  -> CDP websocket
  -> bhd
  -> /tmp/bu-<NAME>.sock
  -> bhrun / bhctl
  -> browser-harness
```

`browser-harness` is intentionally thin. Admin commands are forwarded to
`bhctl`. Runner/helper commands are forwarded to `bhrun`.

## Browser Modes

### Local browser attach

The daemon discovers a locally running Chrome or Edge instance through
`DevToolsActivePort`. This is the default mode for day-to-day usage.

### Remote browser attach

The same runtime can attach to Browser Use cloud browsers by setting:

- `BU_CDP_WS`
- `BU_BROWSER_ID`
- `BROWSER_USE_API_KEY`

`bhctl create-browser` provisions the remote browser and returns `cdpWsUrl`,
`liveUrl`, and the browser id.

## Runtime Files

The daemon runtime is namespaced by `BU_NAME` and uses:

- socket: `/tmp/bu-<name>.sock`
- pid: `/tmp/bu-<name>.pid`
- log: `/tmp/bu-<name>.log`

Useful environment variables:

- `BU_NAME`: daemon namespace, defaults to `default`
- `BU_CDP_WS`: explicit websocket override for remote browsers or pinned local attach
- `BU_BROWSER_ID`: remote browser id for lifecycle cleanup
- `BU_BROWSER_MODE`: `local` or `remote` for smoke and helper flows
- `BU_DAEMON_IMPL`: implementation selector used by repo-local verification

## Guest Model

Guests run through `bhrun`, not directly against the daemon socket.

Current guest layers:

- `rust/guests/*.wat`: minimal WAT examples
- `rust/guests/rust-*`: Rust-to-WASM guests using `bh-guest-sdk`
- `rust/crates/bh-wasm-host`: host boundary between guest calls and the daemon
- `rust/crates/bh-guest-sdk`: guest SDK for typed operations

Guests are capability-gated. A guest config explicitly grants operations such as
`goto`, `js`, `wait_for_load`, `screenshot`, or `cdp_raw`.

## Repository Layout

- `rust/`: binaries, crates, guest modules, and Rust workspace metadata
- `domain-skills/`: site-specific scraping and interaction knowledge
- `interaction-skills/`: reusable browser mechanics
- `docs/`: architecture, development, integration notes, and future design notes
- `install.md`: install/bootstrap entrypoint
- `SKILL.md`: day-to-day operator/agent guide

## Verification Model

The primary acceptance gate is local browser verification through `bhsmoke`.

Typical checks:

- `bhsmoke actions`
- `bhsmoke tabs`
- `bhsmoke guest-run`
- `bhsmoke set-viewport`
- `bhsmoke upload-file`
- `bhsmoke github-trending-guest`
- `bhsmoke 2048-guest`

Remote verification exists, but it is best-effort because it depends on Browser
Use availability and the target sites' live behavior.
