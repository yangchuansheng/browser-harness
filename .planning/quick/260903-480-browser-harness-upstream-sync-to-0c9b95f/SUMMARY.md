---
status: complete
completed: 2026-09-03
slug: upstream-sync
---

# Upstream Sync Summary — 2026-09-03

- Migrated upstream `41108b8..0c9b95f` (58 commits: 35 non-merge + 23 merge) into the Rust daemon, typed CLI, MCP, documentation, and domain architecture in commit `be1af46`.
- Completed a post-push parity audit of daemon health, endpoint redaction, screenshot timeout, relay blank-tab reuse, fail-closed cloud cleanup, live-view suppression, browser launch ownership, and MCP stdio support.
- Bumped the Rust workspace and all workspace-local lockfile packages from `0.1.9` to `0.1.10`.
- Added automatic Shift modifiers for uppercase and shifted printable keys, while preserving Alt/Ctrl/Meta shortcut intent, with focused Rust regression coverage.
- Recorded `fill_input` select-all as structurally non-applicable: Rust clears through JavaScript and inserts through `Input.insertText`, so browser-OS select-all selection is absent from this path.
