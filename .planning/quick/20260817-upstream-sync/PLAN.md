---
status: complete
created: 2026-08-17
slug: upstream-sync
---

# Upstream Sync — 2026-08-17

Migrate upstream `browser-use/browser-harness` commits
`6a80dbbce51e8c1776af061282546627f007be4e..41108b8676d4bdb58b26ab3b079c0b7b0f8f3926`
(11 commits: 9 non-merge + 2 merge) into the Rust fork, preserving the Rust
architecture.

## Scope

- Repo: repository root
- Previous target: `6a80dbbce51e8c1776af061282546627f007be4e`
- New target: `41108b8676d4bdb58b26ab3b079c0b7b0f8f3926`
- Core upstream change: daemon session recovery + named-tab management
  (`src/browser_harness/daemon.py` + `helpers.py`) and a `0.1.9` version bump.
- Docs: SKILL.md, interaction-skills/connection.md, interaction-skills/tabs.md.

## Execution

- Core migration delegated to Codex CLI (full-auto, unattended).
- Rust port target: daemon recovery serialization + tab lifecycle in
  `bh-daemon`/`bhrun`/`bh-wasm-host` where applicable.
- No Python runtime files copied.
- Verification: cargo fmt/check/test, bhrun summary, facade --help,
  git diff --check, Python-fallback secret scan.
- Commit (English conventional, unsigned like prior history) and safe push.
