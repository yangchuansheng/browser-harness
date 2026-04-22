import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path


SUPPRESS_ENV = "BROWSER_HARNESS_SUPPRESS_PY_DEPRECATION"


def _run(cmd, *, cwd=None, env=None):
    return subprocess.run(
        cmd,
        cwd=cwd,
        env=env,
        capture_output=True,
        text=True,
        check=True,
    )


def _python_import_check(python_executable, cwd):
    code = """
import importlib
import json
import warnings

with warnings.catch_warnings(record=True) as caught:
    warnings.simplefilter("always")
    admin = importlib.import_module("admin")
    helpers = importlib.import_module("helpers")

print(json.dumps({
    "admin_file": admin.__file__,
    "helpers_file": helpers.__file__,
    "warning_messages": [str(item.message) for item in caught],
}))
"""
    result = _run([python_executable, "-c", code], cwd=cwd)
    payload = json.loads(result.stdout)
    messages = payload["warning_messages"]
    if not payload["admin_file"].endswith("admin.py"):
        raise AssertionError(f"unexpected admin module path: {payload['admin_file']}")
    if not payload["helpers_file"].endswith("helpers.py"):
        raise AssertionError(f"unexpected helpers module path: {payload['helpers_file']}")
    if not any("`import admin` is deprecated" in message for message in messages):
        raise AssertionError(f"missing admin deprecation warning: {messages}")
    if not any("`import helpers` is deprecated" in message for message in messages):
        raise AssertionError(f"missing helpers deprecation warning: {messages}")
    if not all(f"{SUPPRESS_ENV}=1" in message for message in messages):
        raise AssertionError(f"missing suppression guidance in warnings: {messages}")
    return payload


def _legacy_shell_warning_checks(browser_harness_py, cwd):
    help_proc = _run([browser_harness_py, "--help"], cwd=cwd)
    if "`browser-harness-py` is deprecated" not in help_proc.stderr:
        raise AssertionError(f"missing browser-harness-py warning: {help_proc.stderr}")

    suppressed_env = os.environ.copy()
    suppressed_env[SUPPRESS_ENV] = "1"
    suppressed_proc = _run([browser_harness_py, "--help"], cwd=cwd, env=suppressed_env)
    if "`browser-harness-py` is deprecated" in suppressed_proc.stderr:
        raise AssertionError(f"suppressed browser-harness-py warning still present: {suppressed_proc.stderr}")


def main():
    python_executable = sys.executable
    candidates = [
        Path(python_executable).with_name("browser-harness-py"),
    ]
    which_candidate = shutil.which("browser-harness-py")
    if which_candidate is not None:
        candidates.append(Path(which_candidate))
    browser_harness_py = next((str(path) for path in candidates if path.exists()), None)
    if browser_harness_py is None:
        raise SystemExit("browser-harness-py is not installed in the current environment")

    with tempfile.TemporaryDirectory(prefix="bh-installed-compat-") as tempdir:
        payload = _python_import_check(python_executable, tempdir)
        _legacy_shell_warning_checks(browser_harness_py, tempdir)

    print(
        json.dumps(
            {
                "success": True,
                "admin_file": payload["admin_file"],
                "helpers_file": payload["helpers_file"],
                "warning_count": len(payload["warning_messages"]),
                "browser_harness_py": browser_harness_py,
            }
        )
    )


if __name__ == "__main__":
    main()
