---
status: complete
created: 2026-09-03
slug: upstream-sync
quick_id: 260903-480
---

# Upstream Sync — 2026-09-03

Migrate upstream `browser-use/browser-harness` commits
`41108b8676d4bdb58b26ab3b079c0b7b0f8f3926..0c9b95ff6740556dd71d28f9422a953d203358af`
(58 commits: 35 non-merge + 23 merge)
into the Rust fork, preserving the Rust crate/CLI/WASM architecture.

## Scope

- Analyze all commits and diffs in the target range.
- Replicate the new upstream changes into the Rust architecture; never overwrite the Rust runtime with the Python runtime directly.
- Keep mapping upstream `domain-skills` -> `domains/`, upstream `scraping.md` -> this fork's `skill.md`, and cover the legacy `domain-skills/` path too.
- Update `.planning/migration/upstream-sync-2026-04-21.md` target, commit count, migration summary, and verification evidence.
- Run `cargo fmt/check/test`, CLI smoke, `git diff --check`, and a sensitive-info scan (use `scripts/scan_sensitive_fallback.py` on macOS where `rg`/bash-4 features are unavailable).
- Leave the corrective follow-up in the working tree for parent verification and commit.

## Outcome

- Migration commit `be1af4647f716f33fd1f1e6a3f09395f4012bb59` synced the target range and was pushed before this corrective audit.
- The corrective follow-up aligned the Rust workspace with upstream release `0.1.10`, added printable-key Shift modifier parity with regression coverage, and completed the GSD audit artifacts.
- Python runtime layout and browser-OS select-all logic remain upstream-specific; Rust input clearing uses JavaScript followed by `Input.insertText`.
