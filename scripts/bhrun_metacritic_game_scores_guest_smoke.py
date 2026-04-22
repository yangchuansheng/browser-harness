#!/usr/bin/env python3
"""Run a smoke for the Rust/Wasm Metacritic game-scores guest."""

import json
import os
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO))
os.environ.setdefault("BU_NAME", "bhrun-metacritic-game-scores-guest-smoke")

TARGET_PRODUCT_URL = (
    "https://backend.metacritic.com/games/metacritic/the-last-of-us/web"
    "?componentName=product&componentType=Product&apiKey=1MOZgmNFxvmljaQR1X9KAij9Mo4xAY3u"
)
TARGET_USER_URL = (
    "https://backend.metacritic.com/reviews/metacritic/user/games/the-last-of-us/stats/web"
    "?componentName=user-score-summary&componentType=ScoreSummary&apiKey=1MOZgmNFxvmljaQR1X9KAij9Mo4xAY3u"
)


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
            "failed to build the Rust Metacritic guest; ensure the stable wasm target is installed "
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


def summarize_calls(calls):
    summarized = []
    for call in calls:
        request = call.get("request") or {}
        response = call.get("response")
        entry = {
            "operation": call.get("operation"),
            "url": request.get("url"),
            "timeout": request.get("timeout"),
            "response_length": len(response) if isinstance(response, str) else None,
        }
        if request.get("headers"):
            entry["headers"] = request["headers"]
        summarized.append(entry)
    return summarized


def main():
    name = os.environ["BU_NAME"]
    guest_manifest = REPO / "rust" / "guests" / "rust-metacritic-game-scores" / "Cargo.toml"
    default_guest_path = (
        REPO
        / "rust"
        / "guests"
        / "rust-metacritic-game-scores"
        / "target"
        / "wasm32-unknown-unknown"
        / "release"
        / "rust_metacritic_game_scores_guest.wasm"
    )
    guest_path = Path(os.environ.get("BU_GUEST_PATH", str(default_guest_path)))

    result = {
        "name": name,
        "daemon_impl": os.environ.get("BU_DAEMON_IMPL", "rust"),
        "guest_path": str(guest_path),
        "skill": "domain-skills/metacritic/scraping.md",
        "mode": "http_only",
    }

    if os.environ.get("BU_SKIP_GUEST_BUILD") != "1" and guest_path == default_guest_path:
        build_guest_module(guest_manifest)
        result["guest_manifest"] = str(guest_manifest)
        result["guest_build_mode"] = "cargo+stable"

    sample_config, _ = run_runner_command("sample-config")
    sample_config["daemon_name"] = name
    sample_config["guest_module"] = str(guest_path)
    sample_config["granted_operations"] = ["http_get"]
    sample_config["allow_http"] = True
    result["guest_config"] = sample_config

    guest_run, _ = run_runner_command(
        "run-guest",
        sample_config,
        timeout_seconds=30,
        extra_args=[str(guest_path)],
    )
    calls = guest_run.get("calls") or []
    operations = [call.get("operation") for call in calls]
    result["guest_run"] = {
        "exit_code": guest_run.get("exit_code"),
        "success": guest_run.get("success"),
        "trap": guest_run.get("trap"),
    }
    result["guest_operations"] = operations
    result["guest_calls"] = summarize_calls(calls)

    if not guest_run.get("success") or guest_run.get("exit_code") != 0:
        raise RuntimeError(f"guest run failed: {json.dumps(result, sort_keys=True)}")
    if operations != ["http_get", "http_get"]:
        raise RuntimeError(f"unexpected guest operation sequence: {operations!r}")

    product_call, user_call = calls
    if product_call.get("request", {}).get("url") != TARGET_PRODUCT_URL:
        raise RuntimeError("Metacritic product request URL mismatch")
    if user_call.get("request", {}).get("url") != TARGET_USER_URL:
        raise RuntimeError("Metacritic user request URL mismatch")

    product_payload = json.loads(product_call.get("response") or "{}")
    user_payload = json.loads(user_call.get("response") or "{}")
    product_item = product_payload["data"]["item"]
    user_item = user_payload["data"]["item"]
    result["score_summary"] = {
        "title": product_item.get("title"),
        "platform": product_item.get("platform"),
        "metascore": product_item.get("criticScoreSummary", {}).get("score"),
        "critic_reviews": product_item.get("criticScoreSummary", {}).get("reviewCount"),
        "user_score": user_item.get("score"),
        "user_reviews": user_item.get("reviewCount"),
    }

    if result["score_summary"]["title"] != "The Last of Us":
        raise RuntimeError("unexpected Metacritic title")
    if (result["score_summary"]["metascore"] or 0) < 90:
        raise RuntimeError("unexpected Metacritic critic score")
    if (result["score_summary"]["user_score"] or 0) < 8:
        raise RuntimeError("unexpected Metacritic user score")

    print(json.dumps(result))


if __name__ == "__main__":
    main()
