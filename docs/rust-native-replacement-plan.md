# Rust-Native Replacement Plan

## Purpose

This document defines the path from the current legacy Python compatibility
shell to a fully Rust-native Browser Harness interface.

The goal is not just "Rust under the hood." The goal is:

- Rust owns the runtime and control plane
- Rust exposes the primary top-level CLI
- new browser workflows land on `bhrun` / guests first
- Python compatibility stops receiving new first-class product surface

## Current State

As of the current rewrite stage:

- `bhd` owns the browser runtime
- `bhctl` owns daemon lifecycle, Browser Use control, and profile sync
- `bhrun` owns typed helper operations, event waits, and guest execution
- the new top-level Rust-native facade is `browser-harness` in
  `rust/bins/browser-harness-cli`
- `run.py`, `admin.py`, and `helpers.py` remain only as the legacy
  compatibility shell
- the installed default `browser-harness` command is now the Rust-native CLI,
  while the Python shell is explicitly `browser-harness-py`

This means the Python daemon path is sunset. The remaining Python surface is
now a shim layer, not the product core.

## What "Python Sunset" Means

Python sunset does not require deleting every `.py` file immediately.

It means:

- no new runtime ownership moves into Python
- no new stable helper capability is introduced in Python first
- the top-level product direction is Rust CLI + Rust/Wasm guests
- Python remains compatibility-only until the last actively used workflows are
  replaced

## Replacement Shape

### 1. Top-Level CLI

The new primary control surface should be:

```text
browser-harness
  -> bhctl for admin/control commands
  -> bhrun for helper/runner/guest commands
```

That facade exists now. It is intentionally thin and forwards commands rather
than inventing another manager layer.

### 2. Structured Browser Operations

The primary Rust-native path for stable operations is:

- `browser-harness ensure-daemon`
- `browser-harness daemon-alive`
- `browser-harness restart-daemon`
- `browser-harness current-tab`
- `browser-harness list-tabs`
- `browser-harness new-tab`
- `browser-harness switch-tab`
- `browser-harness ensure-real-tab`
- `browser-harness iframe-target`
- `browser-harness page-info`
- `browser-harness goto`
- `browser-harness wait-for-load`
- `browser-harness js`
- `browser-harness click`
- `browser-harness type-text`
- `browser-harness press-key`
- `browser-harness scroll`
- `browser-harness screenshot`
- `browser-harness wait-for-event`
- `browser-harness watch-events`
- `browser-harness wait-for-response`
- `browser-harness wait-for-console`
- `browser-harness wait-for-dialog`

### 3. Dynamic Task Logic

The long-term replacement for `browser-harness-py <<'PY'` is not another Rust
string-eval shell. It is:

- runner-owned utilities in `bhrun`
- compiled Rust/Wasm guests for reusable task logic
- domain-skill migrations that prove the host boundary

That is the correct replacement because it preserves capability control and
keeps browser ownership in the host.

## What Still Stays Legacy

These remain legacy compatibility only for now:

- `run.py`
- `helpers.py`
- Python convenience wrappers that still sit above the socket contract
- direct ad hoc Python snippets that depend on helper pre-imports

They should only receive:

- critical compatibility fixes
- deprecation-oriented cleanup
- no new primary product features

## Explicitly Deferred To Post-Sunset

Some interaction skills do not need to block the Python sunset itself.

These can land after the Rust-native CLI and guest path become the default:

- dialog handling beyond passive wait coverage
- uploads guest ergonomics
- viewport control
- print-to-PDF
- downloads
- cookies
- drag-and-drop

Those are important, but they are not prerequisites for removing Python as the
primary interface.

## Exit Criteria For Deleting The Python Shell

Python can be removed entirely only when all of the following are true:

- the Rust-native `browser-harness` CLI covers the active install/bootstrap and
  day-to-day control flows
- the remaining production workflows no longer rely on `browser-harness-py <<'PY'`
- raw escape hatch needs are either intentionally dropped or re-homed behind
  `bhrun`
- the interaction/domain backlog no longer depends on Python-only helpers
- the package/install story no longer points at `run.py`

## Immediate Execution Order

1. Make the Rust-native CLI the primary documented control surface.
2. Keep Python compatibility available under an explicit legacy command.
3. Route all new stable capability work to `bhrun` / guests first.
4. Move remaining interaction skills into either:
   - pre-sunset blockers, or
   - explicit post-sunset work
5. Remove Python packaging/runtime dependencies that only existed for
   `daemon.py`.
6. Replace the final public Python entrypoint only after the Rust-native path is
   operationally complete.
