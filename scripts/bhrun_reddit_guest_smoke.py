#!/usr/bin/env python3
"""Run a smoke for the Rust/Wasm Reddit post guest.

Required in remote mode:
  BROWSER_USE_API_KEY

Optional:
  BU_NAME                   defaults to "bhrun-reddit-guest-smoke"
  BU_BROWSER_MODE           defaults to "remote"; set to "local" to attach via DevToolsActivePort
  BU_DAEMON_IMPL            defaults to "rust"
  BU_REMOTE_TIMEOUT_MINUTES defaults to "1"
  BU_LOCAL_DAEMON_WAIT_SECONDS defaults to "15"
  BU_GUEST_PATH             override the guest module path
  BU_SKIP_GUEST_BUILD       set to "1" to skip the default Rust guest build
  BU_RUST_RUNNER_BIN        override the bhrun binary path
"""

import json
import os
import subprocess
import sys
import time
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO))
os.environ.setdefault("BU_NAME", "bhrun-reddit-guest-smoke")

from scripts._admin_cli import (  # noqa: E402
    ensure_daemon,
    list_browsers,
    restart_daemon,
    start_remote_daemon,
)

TARGET_URL_PREFIX = "https://www.reddit.com/r/vibecoding/comments/1kwuqpz"


def poll_browser_status(browser_id, attempts=10, delay=1.0):
    for _ in range(attempts):
        listing = list_browsers(page_size=20, page_number=1)
        item = next((item for item in listing.get("items", []) if item.get("id") == browser_id), None)
        status = item.get("status") if item else "missing"
        if status != "active":
            return status
        time.sleep(delay)
    return status


def runner_process_spec():
    if custom := os.environ.get("BU_RUST_RUNNER_BIN"):
        return [custom], str(REPO)
    return ["cargo", "run", "--quiet", "--bin", "bhrun", "--"], str(REPO / "rust")


def build_guest_module(guest_manifest):
    proc = subprocess.run(
        [
            "cargo",
            "+stable",
            "build",
            "--offline",
            "--release",
            "--target",
            "wasm32-unknown-unknown",
            "--manifest-path",
            str(guest_manifest),
        ],
        cwd=REPO,
        env=os.environ.copy(),
        text=True,
        capture_output=True,
    )
    if proc.returncode != 0:
        detail = (proc.stderr or proc.stdout or "guest build failed").strip()
        raise RuntimeError(
            "failed to build the Rust Reddit guest; ensure the stable wasm target is installed "
            "via `rustup target add --toolchain stable-x86_64-unknown-linux-gnu wasm32-unknown-unknown`"
            f"\n{detail}"
        )


def run_runner_command(subcommand, payload=None, timeout_seconds=10, extra_args=None):
    cmd, cwd = runner_process_spec()
    proc = subprocess.Popen(
        cmd + [subcommand] + (extra_args or []),
        cwd=cwd,
        env=os.environ.copy(),
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    stdin_text = "" if payload is None else json.dumps(payload)
    stdout, stderr = proc.communicate(stdin_text, timeout=timeout_seconds)
    if proc.returncode != 0:
        raise RuntimeError((stderr or stdout or f"bhrun exited {proc.returncode}").strip())
    if not stdout.strip():
        raise RuntimeError("bhrun returned empty stdout")
    return json.loads(stdout), payload


def main():
    browser_mode = os.environ.get("BU_BROWSER_MODE", "remote").strip().lower() or "remote"
    if browser_mode not in {"remote", "local"}:
        raise SystemExit("BU_BROWSER_MODE must be 'remote' or 'local'")
    if browser_mode == "remote" and not os.environ.get("BROWSER_USE_API_KEY"):
        raise SystemExit("BROWSER_USE_API_KEY is required")

    os.environ.setdefault("BU_DAEMON_IMPL", "rust")
    name = os.environ["BU_NAME"]
    timeout = int(os.environ.get("BU_REMOTE_TIMEOUT_MINUTES", "1"))
    local_wait = float(os.environ.get("BU_LOCAL_DAEMON_WAIT_SECONDS", "15"))
    guest_manifest = REPO / "rust" / "guests" / "rust-reddit-post-scrape" / "Cargo.toml"
    default_guest_path = (
        REPO
        / "rust"
        / "guests"
        / "rust-reddit-post-scrape"
        / "target"
        / "wasm32-unknown-unknown"
        / "release"
        / "rust_reddit_post_scrape_guest.wasm"
    )
    guest_path = Path(os.environ.get("BU_GUEST_PATH", str(default_guest_path)))

    browser = None
    result = {
        "name": name,
        "daemon_impl": os.environ["BU_DAEMON_IMPL"],
        "browser_mode": browser_mode,
        "guest_path": str(guest_path),
        "skill": "domain-skills/reddit/scraping.md",
        "target_url_prefix": TARGET_URL_PREFIX,
    }
    try:
        if os.environ.get("BU_SKIP_GUEST_BUILD") != "1" and guest_path == default_guest_path:
            build_guest_module(guest_manifest)
            result["guest_manifest"] = str(guest_manifest)
            result["guest_build_mode"] = "cargo+stable"

        if browser_mode == "remote":
            browser = start_remote_daemon(name=name, timeout=timeout)
            result["browser_id"] = browser["id"]
        else:
            ensure_daemon(name=name, wait=local_wait)
            result["local_attach"] = "DevToolsActivePort"

        sample_config, _ = run_runner_command("sample-config")
        sample_config["daemon_name"] = name
        sample_config["guest_module"] = str(guest_path)
        sample_config["granted_operations"] = [
            "ensure_real_tab",
            "goto",
            "wait_for_load",
            "wait",
            "scroll",
            "page_info",
            "js",
        ]
        result["guest_config"] = sample_config

        guest_run, _ = run_runner_command(
            "run-guest",
            sample_config,
            timeout_seconds=40,
            extra_args=[str(guest_path)],
        )
        result["guest_run"] = guest_run
        calls = guest_run.get("calls") or []
        operations = [call.get("operation") for call in calls]
        result["guest_operations"] = operations

        if not guest_run.get("success"):
            goto_call = next((call for call in calls if call.get("operation") == "goto"), None)
            if goto_call is not None:
                result["failed_goto_response"] = goto_call.get("response")
            try:
                result["page_after_failed_guest"], result["page_after_failed_guest_request"] = (
                    run_runner_command("page-info", {"daemon_name": name})
                )
            except Exception as err:
                result["page_after_failed_guest_error"] = str(err)
            raise RuntimeError(f"guest run failed: {json.dumps(result, sort_keys=True)}")
        if guest_run.get("exit_code") != 0:
            goto_call = next((call for call in calls if call.get("operation") == "goto"), None)
            if goto_call is not None:
                result["failed_goto_response"] = goto_call.get("response")
            try:
                result["page_after_failed_guest"], result["page_after_failed_guest_request"] = (
                    run_runner_command("page-info", {"daemon_name": name})
                )
            except Exception as err:
                result["page_after_failed_guest_error"] = str(err)
            raise RuntimeError(f"unexpected guest exit code: {json.dumps(result, sort_keys=True)}")
        expected_operations = [
            "ensure_real_tab",
            "goto",
            "wait_for_load",
            "wait",
            "scroll",
            "wait",
            "scroll",
            "wait",
            "page_info",
            "js",
        ]
        if operations != expected_operations:
            raise RuntimeError(f"unexpected guest operation sequence: {operations!r}")

        initial_wait = calls[3].get("response") or {}
        first_scroll_wait = calls[5].get("response") or {}
        second_scroll_wait = calls[7].get("response") or {}
        page_response = calls[8].get("response") or {}
        raw_post = calls[9].get("response")

        if int(initial_wait.get("elapsed_ms", 0)) < 3000:
            raise RuntimeError(f"initial guest wait did not sleep long enough: {initial_wait!r}")
        if int(first_scroll_wait.get("elapsed_ms", 0)) < 1000:
            raise RuntimeError(f"first scroll wait was too short: {first_scroll_wait!r}")
        if int(second_scroll_wait.get("elapsed_ms", 0)) < 1000:
            raise RuntimeError(f"second scroll wait was too short: {second_scroll_wait!r}")
        if not str(page_response.get("url", "")).startswith(TARGET_URL_PREFIX):
            raise RuntimeError("guest page_info did not remain on the Reddit post URL")

        post = json.loads(raw_post)
        result["post_sample"] = {
            "subreddit": post.get("subreddit"),
            "title": post.get("title"),
            "author": post.get("author"),
            "score": post.get("score"),
            "comment_count": len(post.get("comments") or []),
            "comment_sample": (post.get("comments") or [])[:3],
            "url": post.get("url"),
            "login_wall": post.get("loginWall"),
            "age_gate": post.get("ageGate"),
        }

        if result["post_sample"]["age_gate"]:
            raise RuntimeError("Reddit guest hit an age gate")
        if result["post_sample"]["subreddit"] != "vibecoding":
            raise RuntimeError(f"unexpected subreddit: {result['post_sample']['subreddit']!r}")
        if not result["post_sample"]["title"]:
            raise RuntimeError("Reddit guest returned an empty post title")
        if not result["post_sample"]["author"]:
            raise RuntimeError("Reddit guest returned an empty post author")
        if result["post_sample"]["comment_count"] < 1:
            raise RuntimeError("Reddit guest did not extract any top-level comments")
        if not str(result["post_sample"]["url"]).startswith(TARGET_URL_PREFIX):
            raise RuntimeError("Reddit guest did not remain on the canonical post URL")

        result["page_after_guest"], result["page_after_guest_request"] = run_runner_command(
            "page-info",
            {"daemon_name": name},
        )
        if not str(result["page_after_guest"].get("url", "")).startswith(TARGET_URL_PREFIX):
            raise RuntimeError("runner page-info after guest did not remain on the Reddit post URL")
    finally:
        restart_daemon(name)
        time.sleep(1)
        if browser is not None:
            result["post_shutdown_status"] = poll_browser_status(browser["id"])
        log_path = Path(f"/tmp/bu-{name}.log")
        if log_path.exists():
            lines = log_path.read_text().strip().splitlines()
            result["log_tail"] = lines[-8:]

    print(json.dumps(result))


if __name__ == "__main__":
    main()
