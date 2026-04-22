# Changelog

## Unreleased

### Breaking Changes

- Installed packages no longer ship `helpers.py` or `admin.py`.
  Prefer the Rust-native `browser-harness` CLI. Historical shim code now lives
  only under `archive/python-legacy/`.
- Installed packages no longer ship `browser-harness-py` or `run.py`.
  The distributed package is now Rust-only.

### Compatibility Notes

- The active source tree no longer keeps repo-local Python shims.
- The old helper-loaded shell, shim modules, and deprecated import aliases now
  live under `archive/python-legacy/`.
- Optional Python examples should shell out to `browser-harness`; see
  `docs/python-integration.md`.
- Installed-package regression coverage now runs through
  `browser-harness verify-install` to check that the Python shell and Python
  compatibility modules are intentionally absent.
