---
status: complete
completed: 2026-08-16
slug: upstream-sync
initial_commit: 50f72656c5ddf70381f35a49d32512671ad837aa
---

# Upstream Sync Summary — 2026-08-16

- Migrated upstream `f5eaf90..6a80dbb` into the Rust CLI architecture.
- Added the typed `mac-approve` admin command, macOS approval automation,
  daemon-ready race handling, diagnostics, docs, and the upstream demo asset.
- Adapted upstream documentation to the Rust workspace and `domains/` layout.
- Parent review corrected setup validation so only Google Chrome's enabled
  remote-debugging toggle satisfies the Google-Chrome-only AppleScript helper.
- Verification passed: Rust fmt/check/test, CLI summary/help/usage smoke,
  whitespace checks, fallback sensitive scan, and local/remote git checks.

Initial migration commit `50f72656c5ddf70381f35a49d32512671ad837aa`
was pushed to `origin/main`; the parent-review correction and completed GSD
artifacts are recorded in the follow-up commit for this quick task.