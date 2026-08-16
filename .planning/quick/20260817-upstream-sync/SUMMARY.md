---
status: complete
completed: 2026-08-17
slug: upstream-sync
---

# Upstream Sync Summary — 2026-08-17

- Migrated upstream `6a80dbb..41108b8` into the Rust daemon and typed runner architecture.
- Added reusable named-daemon background targets, serialized exact-session recovery, shared replacement sessions, and explicit tab activation.
- Added backward-compatible `activate` request handling through `bh-wasm-host`, `bhrun`, and `bh-guest-sdk`.
- Adapted the root and interaction skills to the Rust CLI and background-tab model.
- Bumped the Rust workspace from `0.1.8` to `0.1.9` and preserved 109/109 domain-skill mappings.
- Verification passed: workspace fmt/check/test with 198 tests, 42-operation runner summary, facade help smoke, 109/109 domain coverage, whitespace checks, and a differential sensitive scan with 0 new hits.
