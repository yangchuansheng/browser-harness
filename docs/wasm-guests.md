# WASM Guests

This document describes the guest model that exists in Browser Harness today.

A guest is a WebAssembly module that runs inside `bhrun` and calls host
operations through the WASM host boundary. It is the unit you use when you want
to package browser workflow logic as code without moving browser lifecycle or
CDP ownership into the guest itself.

## What A Guest Is

The runtime shape is:

```text
WASM guest
  -> bhrun
  -> bh-wasm-host
  -> bhd
  -> CDP / Browser Use
```

That division of responsibility matters:

- `bhd` owns the live browser websocket and session state
- `bhctl` owns browser lifecycle and control-plane operations
- `bhrun` owns guest execution, capability enforcement, and guest-host calls
- the guest owns workflow logic only

A guest does not:

- connect to the daemon socket directly
- provision or stop browsers
- own daemon lifecycle
- bypass host capability checks

## Why Use A Guest

Guests are useful when you want to package repeatable browser logic as a module
instead of pasting a long sequence of CLI calls into a shell script or agent
prompt.

Typical use cases:

- domain-specific scraping workflows
- repeatable interaction sequences that need local state
- capability-gated automation for agents
- portable logic that can be run locally or against a remote browser with the
  same host surface

Guests are not a replacement for the host CLI. They sit on top of it.

## Guest Types In This Repo

The repo currently has two guest styles under [`rust/guests/`](../rust/guests/):

- WAT examples such as [`navigate_and_read.wat`](../rust/guests/navigate_and_read.wat) and [`persistent_counter.wat`](../rust/guests/persistent_counter.wat)
- Rust-to-WASM guests such as [`rust-navigate-and-read`](../rust/guests/rust-navigate-and-read), [`rust-github-trending`](../rust/guests/rust-github-trending), and [`rust-2048-autoplay`](../rust/guests/rust-2048-autoplay)

WAT is useful for the smallest possible examples. Rust is the normal path for
real guests because it can use `bh-guest-sdk`.

## Execution Modes

There are two main ways to run a guest.

### `run-guest`

`run-guest` executes one guest invocation and exits.

Use it when:

- you want a single run
- you do not need WASM guest memory to persist between runs
- you want the simplest execution model

Example:

```bash
cd rust
cargo run --quiet --bin bhrun -- run-guest guests/navigate_and_read.wat <<'JSON'
{"daemon_name":"default","guest_module":"guests/navigate_and_read.wat","granted_operations":["goto","wait_for_load_event","page_info","js"],"allow_http":false,"allow_raw_cdp":false,"persistent_guest_state":true}
JSON
```

### `serve-guest`

`serve-guest` starts a persistent guest runtime and accepts NDJSON commands on
stdin. This is the mode to use when you want the guest's internal WASM state to
survive multiple `run` commands.

Use it when:

- you want multiple invocations against the same guest runtime
- the guest keeps state in globals or memory
- you want a runner-style guest session

Example:

```bash
cd rust
cat <<'NDJSON' | cargo run --quiet --bin bhrun -- serve-guest guests/persistent_counter.wat
{"command":"start","config":{"daemon_name":"default","guest_module":"guests/persistent_counter.wat","granted_operations":["wait"],"allow_http":false,"allow_raw_cdp":false,"persistent_guest_state":true}}
{"command":"run"}
{"command":"run"}
{"command":"status"}
{"command":"stop"}
NDJSON
```

`serve-guest` requires `persistent_guest_state=true`.

## `serve-guest` Protocol

`serve-guest` is a line-oriented NDJSON protocol.

Supported request commands:

- `start`
- `run`
- `status`
- `stop`

Typical flow:

1. send `start` with guest config
2. receive a `ready` response
3. send one or more `run` commands
4. optionally send `status`
5. send `stop`

Typical response kinds:

- `ready`
- `run_result`
- `status`
- `stopped`

That protocol is useful when you want to keep one guest runtime alive and drive
it from another process without rebuilding the guest each time.

## Configuration Model

Guests run with a `RunnerConfig`. You can inspect the current shape with:

```bash
cd rust
cargo run --quiet --bin bhrun -- sample-config
```

The key fields are:

- `daemon_name`: which daemon/browser session to use
- `guest_module`: path to the `.wasm` or `.wat` guest module
- `granted_operations`: explicit allow-list of host operations the guest may call
- `allow_http`: whether the guest may use `http_get`
- `allow_raw_cdp`: whether the guest may use `cdp_raw`
- `persistent_guest_state`: whether the runtime should preserve guest state across runs

Two details matter here:

- `granted_operations` is the main capability allow-list
- `allow_http` and `allow_raw_cdp` are extra gates, not substitutes for the allow-list

So `cdp_raw` only works if:

- `cdp_raw` is present in `granted_operations`
- `allow_raw_cdp` is `true`

The same pattern applies to `http_get`.

## Available Operations

You can inspect the current operation surface with:

```bash
cd rust
cargo run --quiet --bin bhrun -- manifest
cargo run --quiet --bin bhrun -- capabilities
```

Today the host surface is grouped into three practical layers:

### Compatibility helpers

These are the stable browser operations guests use most often:

- tab/session helpers
- `goto`
- `wait_for_load`
- `js`
- input helpers such as click, scroll, type, and keys
- viewport, screenshot, PDF, upload, cookies, and download configuration

### Host utilities

These are runner-owned utilities that are useful but not page-specific:

- `wait`
- `wait_for_event`
- `watch_events`
- `wait_for_load_event`
- `wait_for_download`
- `wait_for_request`
- `wait_for_response`
- `wait_for_console`
- `wait_for_dialog`
- `http_get`

### Escape hatch

`cdp_raw` exists for gaps in the typed surface, but it is intentionally:

- explicit
- capability-gated
- disabled by default

For new guests, prefer stable helpers first.

## Writing A Rust Guest

Most real guests in this repo use [`bh-guest-sdk`](../rust/crates/bh-guest-sdk).

The smallest shape looks like this:

```rust
use bh_guest_sdk::{goto, js, page_info, wait_for_load_event};
use serde_json::Value;

#[no_mangle]
pub extern "C" fn run() -> i32 {
    match run_inner() {
        Ok(()) => 0,
        Err(code) => code,
    }
}

fn run_inner() -> Result<(), i32> {
    goto("https://example.com").map_err(|_| 1)?;

    let load = wait_for_load_event(5_000, 100).map_err(|_| 2)?;
    if !load.matched {
        return Err(2);
    }

    let page = page_info().map_err(|_| 3)?;
    if page.get("url").and_then(Value::as_str) != Some("https://example.com/") {
        return Err(4);
    }

    let title: String = js("document.title").map_err(|_| 5)?;
    if title.is_empty() {
        return Err(6);
    }

    Ok(())
}
```

Important details:

- the guest exports `run`
- success is exit code `0`
- guest-defined nonzero exit codes are the simplest failure contract
- SDK calls return `Result<_, GuestError>`

The SDK currently exposes helpers for:

- tab/session access
- navigation and page inspection
- JavaScript evaluation
- mouse and keyboard input
- viewport, screenshot, PDF, downloads, uploads, and cookies
- event waits and HTTP access
- raw CDP when explicitly allowed

## Building A Rust Guest

Install the WASM target once:

```bash
rustup target add --toolchain stable-x86_64-unknown-linux-gnu wasm32-unknown-unknown
```

Build a guest:

```bash
cd rust
cargo +stable build --release --target wasm32-unknown-unknown --manifest-path guests/rust-github-trending/Cargo.toml
```

That produces a `.wasm` artifact under the guest crate's target directory.

## Running A Rust Guest

Example with the Rust navigate-and-read sample:

```bash
cd rust
cargo +stable build --release --target wasm32-unknown-unknown --manifest-path guests/rust-navigate-and-read/Cargo.toml
cargo run --quiet --bin bhrun -- run-guest guests/rust-navigate-and-read/target/wasm32-unknown-unknown/release/rust_navigate_and_read_guest.wasm <<'JSON'
{"daemon_name":"default","guest_module":"guests/rust-navigate-and-read/target/wasm32-unknown-unknown/release/rust_navigate_and_read_guest.wasm","granted_operations":["goto","wait_for_load_event","page_info","js"],"allow_http":false,"allow_raw_cdp":false,"persistent_guest_state":true}
JSON
```

The result includes:

- `exit_code`
- `success`
- the host calls the guest made
- trap information if the guest runtime failed unexpectedly

## State Model

There are two kinds of state to think about.

### Guest runtime state

This is the guest's own WASM memory and globals.

- `run-guest`: fresh runtime each invocation
- `serve-guest`: persistent runtime across `run` commands

The WAT example [`persistent_counter.wat`](../rust/guests/persistent_counter.wat) demonstrates this model directly.

### Browser state

This is the page/session/browser state visible through the host runtime.

Examples:

- current tab
- DOM state
- cookies
- JavaScript values stored in the page

The Rust sample [`rust-persistent-browser-state`](../rust/guests/rust-persistent-browser-state) demonstrates that a guest can rely on browser state across later runs when it is using the same daemon/browser session.

## When To Put Logic In A Guest

A guest is a good fit when:

- the logic is workflow-shaped, not just one host call
- you want the behavior packaged and rerunnable
- the workflow benefits from guest-local state
- the operation set is already stable enough to expose as capabilities

Keep logic in the host/CLI when:

- it changes browser lifecycle
- it changes daemon lifecycle
- it is broadly reusable infrastructure rather than workflow logic
- it is better modeled as a new stable host operation than as repeated guest-side raw CDP

## Practical Workflow

For a new guest, the usual loop is:

1. Inspect the current host surface with `bhrun manifest` and `bhrun capabilities`.
2. Start from an existing guest under [`rust/guests/`](../rust/guests/).
3. Grant the smallest operation set that works.
4. Prefer typed helpers before enabling `cdp_raw`.
5. Use `run-guest` first.
6. Move to `serve-guest` only when you actually need persistent guest state.

## Related Docs

- [architecture.md](architecture.md)
- [development.md](development.md)
- [future-wasm.md](future-wasm.md)
- [../rust/README.md](../rust/README.md)
