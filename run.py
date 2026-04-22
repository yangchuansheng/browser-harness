import sys
import warnings

from admin_cli import (
    ensure_daemon,
    list_cloud_profiles,
    list_local_profiles,
    restart_daemon,
    start_remote_daemon,
    stop_remote_daemon,
    sync_local_profile,
)
from legacy_warnings import LegacyPythonSurfaceWarning, warn_legacy_surface

HELP = """Browser Harness

Read SKILL.md for the default workflow and examples.

Legacy Python-shell usage:
  browser-harness-py <<'PY'
  ensure_real_tab()
  print(page_info())
  PY

Helpers are pre-imported. The daemon auto-starts and connects to the running
browser.
The primary command is now the Rust-native `browser-harness` CLI.
"""


def _preload_helpers():
    with warnings.catch_warnings():
        warnings.simplefilter("ignore", LegacyPythonSurfaceWarning)
        import helpers

    globals().update({name: getattr(helpers, name) for name in helpers.__all__})


def main():
    command = sys.argv[0] or "browser-harness-py"
    warn_legacy_surface(
        "`browser-harness-py` is deprecated; use the Rust-native `browser-harness` command instead."
    )
    if len(sys.argv) > 1 and sys.argv[1] in {"-h", "--help"}:
        print(HELP)
        return
    if sys.stdin.isatty():
        sys.exit(
            f"{command} reads Python from stdin. Use:\n"
            f"  {command} <<'PY'\n"
            "  print(page_info())\n"
            "  PY"
        )
    _preload_helpers()
    ensure_daemon()
    exec(sys.stdin.read())


if __name__ == "__main__":
    main()
