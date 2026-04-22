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

def import_status(name):
    try:
        importlib.import_module(name)
    except ModuleNotFoundError:
        return "missing"
    return "present"

run = importlib.import_module("run")
run._preload_helpers()
print(json.dumps({
    "admin_status": import_status("admin"),
    "helpers_status": import_status("helpers"),
    "has_cdp": callable(getattr(run, "cdp", None)),
    "has_goto": callable(getattr(run, "goto", None)),
    "has_wait": callable(getattr(run, "wait", None)),
    "wait_module": getattr(getattr(run, "wait", None), "__module__", None),
}))
"""
    result = _run([python_executable, "-c", code], cwd=cwd)
    payload = json.loads(result.stdout)
    if payload["admin_status"] != "missing":
        raise AssertionError(f"admin should be absent from the installed package: {payload}")
    if payload["helpers_status"] != "missing":
        raise AssertionError(f"helpers should be absent from the installed package: {payload}")
    if not payload["has_cdp"] or not payload["has_goto"] or not payload["has_wait"]:
        raise AssertionError(f"run.py did not preload the expected fallback helper surface: {payload}")
    if payload["wait_module"] != "runner_cli":
        raise AssertionError(f"unexpected wait binding after fallback preload: {payload}")
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
                "admin_status": payload["admin_status"],
                "helpers_status": payload["helpers_status"],
                "has_cdp": payload["has_cdp"],
                "browser_harness_py": browser_harness_py,
            }
        )
    )


if __name__ == "__main__":
    main()
