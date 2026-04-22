import sys

from admin_cli import (
    ensure_daemon,
    list_cloud_profiles,
    list_local_profiles,
    restart_daemon,
    start_remote_daemon,
    stop_remote_daemon,
    sync_local_profile,
)
from helpers import *

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


def main():
    command = sys.argv[0] or "browser-harness-py"
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
    ensure_daemon()
    exec(sys.stdin.read())


if __name__ == "__main__":
    main()
