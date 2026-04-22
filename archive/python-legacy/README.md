## Python Legacy Archive

This directory keeps the removed root-level Python compatibility layer for
historical reference only.

Archived here:

- `run.py`
- `helpers.py`
- `admin.py`
- `legacy_warnings.py`
- `runner_cli.py`
- `admin_cli.py`
- `scripts/_runner_cli.py`
- `scripts/_admin_cli.py`

These files are no longer part of the active source-tree workflow, CI gate, or
documented install path.

Prefer the Rust-native CLIs for all new work:

- `browser-harness`
- `bhrun`
- `bhctl`
- `bhsmoke`

If you still want Python around the Rust CLI, use the subprocess wrappers in
`docs/python-cli-helpers.md` instead of importing anything from this archive.
