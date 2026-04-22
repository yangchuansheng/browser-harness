import os
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

_COMPAT_INTERNAL = (
    "chrome://",
    "chrome-untrusted://",
    "devtools://",
    "chrome-extension://",
    "about:",
)
_RUNNER_EXPORTS = (
    "click",
    "current_tab",
    "dispatch_key",
    "drain_events",
    "ensure_real_tab",
    "goto",
    "http_get",
    "iframe_target",
    "js",
    "list_tabs",
    "new_tab",
    "page_info",
    "press_key",
    "screenshot",
    "scroll",
    "switch_tab",
    "type_text",
    "upload_file",
    "wait_for_load",
)

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
        try:
            import helpers
        except ModuleNotFoundError as err:
            if err.name != "helpers":
                raise
            import runner_cli

            def cdp(method, session_id=None, **params):
                return runner_cli.cdp_raw(method, params or None, session_id=session_id)

            globals().update(
                {
                    name: getattr(runner_cli, name)
                    for name in _RUNNER_EXPORTS
                }
            )
            globals().update(
                {
                    "INTERNAL": _COMPAT_INTERNAL,
                    "NAME": os.environ.get("BU_NAME", "default"),
                    "SOCK": f"/tmp/bu-{os.environ.get('BU_NAME', 'default')}.sock",
                    "cdp": cdp,
                    "wait": runner_cli.wait_compat,
                }
            )
            return

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
