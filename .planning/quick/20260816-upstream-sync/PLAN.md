---
status: complete
created: 2026-08-16
slug: upstream-sync
---

# Upstream Sync — 2026-08-16

Migrate upstream `browser-use/browser-harness` commits `f5eaf90..6a80dbb`
(21 commits: 15 non-merge + 6 merge) into the Rust fork, preserving the Rust
architecture.

## Scope

- Repo: repository root
- Previous target: `f5eaf904b221dde0118eba1496961c3dc20fda88`
- New target: `6a80dbbce51e8c1776af061282546627f007be4e`
- Core upstream change: new macOS `mac-approve` remote-debugging approval
  helper (`src/browser_harness/macos.py` + `admin.py`/`run.py` wiring).
- Docs: SKILL.md, install.md, README.md (X video showcase), CONTRIBUTING.md,
  and a new binary GIF asset.

## Execution

- Core migration delegated to Codex CLI (full-auto, unattended).
- Rust port: `mac-approve` in `bhctl` + facade; `daemon_browser_ready` via
  `already_running`; ensure-daemon macOS hint.
- No Python runtime files copied.
- Verification: cargo fmt/check/build/test, bhrun summary, facade --help,
  git diff --check, Python-fallback secret scan.
- Commit (English conventional, unsigned like prior history) and safe push.
