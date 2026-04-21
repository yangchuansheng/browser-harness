# Rust Rewrite Plan

## Goal

Rewrite the stable runtime parts of Browser Harness in Rust without breaking the current user-facing workflow:

- keep `browser-harness <<'PY'` as the short-term interface
- keep `helpers.py` and `admin.py` as thin dynamic wrappers first
- replace the Python daemon/core with a Rust daemon
- later replace the Python dynamic layer with Rust-hosted WASM modules

The rewrite should be incremental, not a clean-room replacement.

## Repository Strategy

Short term: do the rewrite in this repository.

Reasons:

- the current Python entrypoints, docs, skills, and install flow all live here
- compatibility is easier to verify when the old and new paths share one tree
- the current socket boundary is already a clean seam for swapping the daemon
- splitting too early would slow down iteration and create documentation drift

Recommended shape:

```text
.
├── admin.py
├── daemon.py              # legacy until Rust daemon replaces it
├── helpers.py
├── run.py
├── docs/
├── domain-skills/
├── interaction-skills/
└── rust/
    ├── Cargo.toml
    ├── crates/
    │   ├── bh-protocol
    │   ├── bh-discovery
    │   ├── bh-cdp
    │   ├── bh-daemon
    │   ├── bh-remote
    │   └── bh-wasm-host      # later
    └── bins/
        ├── bhd               # daemon
        ├── bhctl             # admin/control
        └── bhrun             # later, WASM/script runner
```

Long term: only consider splitting crates into a separate repo if the Rust core becomes independently reusable and stable enough to justify separate release/versioning.

## Architecture Targets

### Phase 1 target

```text
Python script -> Python wrappers -> Rust daemon/core -> CDP -> local Chrome or Browser Use cloud
```

This preserves the current workflow while moving stateful runtime logic into Rust.

### Phase 1 status

Phase 1 is complete for the in-repo hybrid rewrite.

That means the boundary freeze below has been reached:

- Rust owns the stateful browser runtime, admin lifecycle, remote-browser lifecycle, and the common typed helper surface
- Python remains the compatibility shell and keeps the intentional leftovers (`cdp()`, `wait()`, `http_get()`, dynamic skills)
- existing workflows have live acceptance coverage through the Rust path, including a domain-skill acceptance smoke for `domain-skills/github/scraping.md`

Still desirable, but no longer blocking Phase 2 work:

- one local Chrome/Edge attach acceptance smoke for the `DevToolsActivePort` path

### Phase 2 target

```text
WASM guest -> Rust host runner -> Rust daemon/core -> CDP -> local Chrome or Browser Use cloud
```

This removes the Python dynamic layer after the host API is proven stable.

More specifically, the target shape is:

```text
WASM guest
  -> bhrun (persistent runner)
  -> bh-wasm-host (capability catalog + host-call routing)
  -> bhd
```

The runner stays separate from the daemon so guest failures cannot kill the
browser session.

## What Moves To Rust First

These parts are stable, stateful, and worth compiling early:

- local browser discovery via `DevToolsActivePort`
- CDP websocket connection and raw method forwarding
- daemon socket server and request routing
- session ownership and stale-session recovery
- event buffering and dialog tracking
- tab attach/switch/create primitives
- screenshot, mouse, keyboard, scroll, file upload primitives
- daemon lifecycle and shutdown behavior
- Browser Use cloud browser create/stop flow

These correspond mainly to the current responsibilities in `daemon.py` and the lifecycle portions of `admin.py`.

## What Stays Dynamic First

These parts should remain Python for the POC:

- stdin execution model in `run.py`
- public helper names in `helpers.py`
- convenience helpers and small JS snippets
- exploratory or experimental helpers
- site-specific logic in `domain-skills/`
- reusable interaction guidance in `interaction-skills/`
- raw escape hatches like `js(...)` and `cdp(...)`

Rule of thumb: if a helper is still changing because agents are discovering the right behavior, keep it dynamic. If it is stable and stateful, move it into Rust.

### Phase 1 boundary freeze

The end of Phase 1 is not "zero Python helpers." The boundary is considered
stable when:

- Rust owns the stateful browser runtime, admin lifecycle, remote-browser
  lifecycle, and the common typed helper surface
- `helpers.py` remains as a thin compatibility shell
- `cdp()` is kept intentionally as the raw escape hatch
- `wait()` and `http_get()` remain intentionally client/runner-side utilities
- unsupported typed meta commands degrade through the explicit
  `unsupported meta command: <name>` contract instead of forcing immediate Rust
  ports

Reaching that state marks the end of the short-term rewrite. Starting Phase 2
does not require migrating those intentional leftovers into `bhd`.

## Short-Term Rewrite Plan

### 1. Freeze compatibility

Define the behavior that must not change during the daemon swap:

- socket path naming via `BU_NAME`
- one-line JSON request/response protocol
- meta commands: `drain_events`, `session`, `set_session`, `pending_dialog`, `shutdown`
- helper semantics for `goto`, `page_info`, `new_tab`, `switch_tab`, `list_tabs`, `click`, `type_text`, `press_key`, `scroll`, `screenshot`, `js`, `upload_file`

Add compatibility tests around the current protocol before replacing the daemon.

### 2. Introduce a Rust workspace in this repo

Create a `rust/` workspace with these initial crates:

- `bh-protocol`: serde models for daemon requests/responses
- `bh-discovery`: local Chrome/Edge profile discovery and socket/pid/log paths
- `bh-cdp`: websocket transport and CDP request plumbing
- `bh-daemon`: daemon state machine, session management, event handling
- `bh-remote`: Browser Use REST client

Initial binaries:

- `bhd`: long-lived daemon
- `bhctl`: admin/control helper for start/stop/health

### 3. Replace `daemon.py` with `bhd`

Swap only the daemon first. Keep Python wrappers in place.

Requirements:

- preserve the current Unix socket protocol
- preserve `BU_NAME` namespacing
- preserve stale-session recovery behavior
- preserve remote-browser shutdown behavior
- preserve current log/pid cleanup behavior

At this stage `run.py`, `helpers.py`, and most of `admin.py` should still work with minimal changes.

### 4. Make Python wrappers thin

Keep the existing helper names but reduce Python logic where possible:

- `helpers.py` becomes a compatibility facade over the daemon protocol
- `admin.py` starts using `bhctl` or direct daemon IPC where appropriate
- avoid adding new stateful logic to Python once Rust equivalents exist

The goal is to preserve ergonomics while moving ownership into Rust.

### 5. Move admin and remote functions to Rust

Port lifecycle and Browser Use functions behind `bhctl`:

- ensure daemon
- restart daemon
- start remote daemon
- stop remote daemon
- list cloud profiles

`sync_local_profile` can remain Python briefly if it still shells out cleanly, then move later if needed.

### 6. Add verification

Verification should cover:

- local browser attach
- no-real-tab behavior creates `about:blank`
- tab switching and visible target activation
- `page_info()` and `js(...)`
- screenshot capture
- input primitives
- stale-session recovery
- remote browser startup/shutdown
- at least one unchanged domain-skill workflow through the Rust compatibility path

Keep the smallest meaningful tests first, then add broader integration coverage.

## Long-Term WASM Plan

Do not replace Python with WASM until the Rust daemon and host API are stable.

### WASM design principles

- WASM is for task logic, not daemon ownership
- Rust host owns browser state, sockets, tabs, CDP, retries, and shutdown
- guest modules get a small capability-based host API
- generated CDP bindings should be first-class, not an afterthought
- the guest boundary should be protocol-first, with helpers layered on top
- `bhrun` should be persistent so guest state can survive across calls
- event primitives are more durable than adding many polling helpers
- keep an explicit escape hatch for raw CDP, but make it deliberate and capability-gated

### Proposed WASM host API

The host boundary should have two layers.

Generated protocol families:

- `cdp.browser_protocol`
- `cdp.js_protocol`

Host utilities:

- `wait(seconds)`
- `wait_for_event(filter, timeout)`
- `http_get(url, headers)`

Compatibility helpers kept for ergonomics while the product transitions:

- `page_info()`
- `new_tab(url)`
- `goto(url)`
- `click(x, y)`
- `type_text(text)`
- `press_key(key, modifiers)`
- `dispatch_key(selector, key, event)`
- `scroll(x, y, dx, dy)`
- `screenshot(full)`
- `wait_for_load(timeout)`
- `list_tabs()`
- `switch_tab(target_id)`
- `js(expression)`

Escape hatch:

- `cdp_raw(method, params)` as an advanced, capability-gated fallback

### WASM boundary

Prefer this shape:

```text
WASM guest module
  -> host functions exposed by bhrun
  -> daemon socket
  -> bhd
```

Do not let the guest own the daemon connection directly. Keep the guest separate from the daemon so guest failures cannot kill the browser session.

Current design scaffold:

- [docs/wasm-runner-design.md](/home/allosaurus/Workspace/browser-harness/docs/wasm-runner-design.md)
- `bhrun manifest`
- `bhrun sample-config`

## Migration Milestones

### Milestone A: Rust daemon POC

Success means:

- existing `browser-harness <<'PY'` scripts still run
- Rust daemon handles core helpers reliably
- common attach/navigation/screenshot/click flows work

### Milestone B: Rust-first admin

Success means:

- daemon lifecycle is Rust-owned
- remote-browser lifecycle is Rust-owned
- Python admin layer is thin

### Milestone C: Stable host API

Success means:

- helper surface has settled enough to define a durable host boundary
- new dynamic logic rarely needs new daemon internals

### Milestone D: WASM runner

Success means:

- basic guest modules can drive browser tasks through host capabilities
- dynamic logic can move out of Python without expanding the host surface too much

### Milestone E: Python sunset decision

Only after Milestone D should the project decide whether to:

- keep Python as an optional compatibility mode
- replace Python entirely with WASM guests
- support both for a period

## Risks

- reproducing current session/tab edge cases exactly
- freezing the wrong host API too early
- making the WASM ABI too low-level and unstable
- removing the dynamic escape hatch before the Rust core is fully proven
- introducing two competing public interfaces instead of one layered system

## Recommended Rewrite Order

1. Add compatibility tests around the current daemon protocol.
2. Introduce the Rust workspace in this repository.
3. Implement `bhd` and keep the current socket contract.
4. Keep `run.py`, `helpers.py`, and most of `admin.py` as compatibility wrappers.
5. Move lifecycle and remote-browser operations into `bhctl`.
6. Stabilize the host API based on actual use, not speculation.
7. Add a separate WASM runner.
8. Migrate proven dynamic logic from Python to WASM incrementally.

## Final Recommendation

Rewrite the project in this repository first.

Use a side-by-side migration:

- Python remains the compatibility shell
- Rust becomes the runtime core
- WASM becomes the future dynamic layer only after the Rust host boundary is stable

That path preserves the current product while still moving toward a Rust + WASM architecture.
