# Changelog

## Unreleased

### Breaking Changes

- Installed packages no longer ship `helpers.py` or `admin.py`.
  Use `runner_cli.py` for stable Python helper calls, `admin_cli.py` for
  Python admin/control helpers, or prefer the Rust-native `browser-harness`
  CLI.
- Installed packages no longer ship `browser-harness-py` or `run.py`.
  The distributed package is now Rust-only.

### Compatibility Notes

- The source tree still keeps `run.py` as a deprecated repo-local shell.
- The source tree still keeps `helpers.py` and `admin.py` for repo-local
  compatibility coverage and deprecation testing.
- Installed-package regression coverage now runs through
  `browser-harness verify-install` to check that the Python shell and Python
  compatibility modules are intentionally absent.
