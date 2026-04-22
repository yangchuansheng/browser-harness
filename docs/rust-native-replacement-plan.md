# Rust-Native Replacement Plan

Status: complete. This file now documents the replacement outcome and the
remaining repo-local compatibility boundary, rather than an in-progress
transition.

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
- `run.py`, `admin_cli.py`, `runner_cli.py`, and `helpers.py` remain the
  legacy compatibility shell surface
- `admin.py` is only a compatibility alias to `admin_cli.py`
- the installed package is now Rust-only and the default installed command is
  `browser-harness`
- `run.py`, `helpers.py`, and `admin.py` are now explicitly
  deprecated and emit suppressible warnings
- repo-owned smoke/verification scripts now call `browser-harness` / `bhrun`
  through small script shims instead of importing `helpers.py` or `admin.py`
- installed-package regression coverage now explicitly checks that installed
  packages omit the deprecated Python shell and compatibility modules

This means the Python daemon path is sunset. The remaining Python surface is
now repo-local only, not the product core.

The raw `helpers.py` `cdp()` escape hatch now also has a Rust-native
replacement via `browser-harness cdp-raw` and `bh_guest_sdk::cdp_raw(...)`.

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
- `browser-harness dispatch-key`
- `browser-harness scroll`
- `browser-harness screenshot`
- `browser-harness handle-dialog`
- `browser-harness upload-file`
- `browser-harness wait-for-event`
- `browser-harness watch-events`
- `browser-harness wait-for-response`
- `browser-harness wait-for-console`
- `browser-harness wait-for-dialog`

### 3. Dynamic Task Logic

The long-term replacement for `python3 run.py <<'PY'` is not another Rust
string-eval shell. It is:

- runner-owned utilities in `bhrun`
- compiled Rust/Wasm guests for reusable task logic
- domain-skill migrations that prove the host boundary

That is the correct replacement because it preserves capability control and
keeps browser ownership in the host.

## What Still Stays Legacy

These remain legacy compatibility only for now:

- `run.py`
- `runner_cli.py`
- `helpers.py`
- `admin_cli.py`
- `admin.py` as a thin alias only
- Python convenience wrappers that still sit above the socket contract
- direct ad hoc Python snippets that depend on helper pre-imports

Those are compatibility shims now, not the canonical product surface. Raw CDP
is no longer Python-only.

Deprecation policy inside that legacy surface:

- `runner_cli.py` and `admin_cli.py` are the only Python shims still intended
  to remain canonical during compatibility mode
- installed packages no longer ship the Python shell or compatibility modules
- `run.py`, `helpers.py`, and `admin.py` are deprecated and warn by default in
  the source tree
- `BROWSER_HARNESS_SUPPRESS_PY_DEPRECATION=1` suppresses those warnings for
  legacy automation only
- `run.py`, `helpers.py`, `admin.py`, `runner_cli.py`, and `admin_cli.py`
  remain repo-local only for compatibility tests and source-tree fallback
  paths

They should only receive:

- critical compatibility fixes
- deprecation-oriented cleanup
- no new primary product features

## Explicitly Deferred To Post-Sunset

Some interaction skills do not need to block the Python sunset itself.

These can land after the Rust-native CLI and guest path become the default:

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
- the remaining production workflows no longer rely on `python3 run.py <<'PY'`
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
