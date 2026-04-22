import json
import shutil
import subprocess
import sys
import tempfile


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

print(json.dumps({
    "run_status": import_status("run"),
    "admin_status": import_status("admin"),
    "admin_cli_status": import_status("admin_cli"),
    "helpers_status": import_status("helpers"),
    "runner_cli_status": import_status("runner_cli"),
    "legacy_warnings_status": import_status("legacy_warnings"),
}))
"""
    result = _run([python_executable, "-c", code], cwd=cwd)
    payload = json.loads(result.stdout)
    if payload["run_status"] != "missing":
        raise AssertionError(f"run should be absent from the installed package: {payload}")
    if payload["admin_status"] != "missing":
        raise AssertionError(f"admin should be absent from the installed package: {payload}")
    if payload["admin_cli_status"] != "missing":
        raise AssertionError(f"admin_cli should be absent from the installed package: {payload}")
    if payload["helpers_status"] != "missing":
        raise AssertionError(f"helpers should be absent from the installed package: {payload}")
    if payload["runner_cli_status"] != "missing":
        raise AssertionError(f"runner_cli should be absent from the installed package: {payload}")
    if payload["legacy_warnings_status"] != "missing":
        raise AssertionError(f"legacy_warnings should be absent from the installed package: {payload}")
    return payload


def main():
    python_executable = sys.executable
    browser_harness_py = shutil.which("browser-harness-py")
    if browser_harness_py is not None:
        raise AssertionError(f"browser-harness-py should be absent from the installed package: {browser_harness_py}")

    with tempfile.TemporaryDirectory(prefix="bh-installed-compat-") as tempdir:
        payload = _python_import_check(python_executable, tempdir)

    print(
        json.dumps(
            {
                "success": True,
                "run_status": payload["run_status"],
                "admin_status": payload["admin_status"],
                "admin_cli_status": payload["admin_cli_status"],
                "helpers_status": payload["helpers_status"],
                "runner_cli_status": payload["runner_cli_status"],
                "legacy_warnings_status": payload["legacy_warnings_status"],
                "browser_harness_py": None,
            }
        )
    )


if __name__ == "__main__":
    main()
