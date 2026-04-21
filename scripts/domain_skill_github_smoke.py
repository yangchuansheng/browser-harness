#!/usr/bin/env python3
"""Run a live acceptance smoke for domain-skills/github/scraping.md in Rust mode.

Required:
  BROWSER_USE_API_KEY

Optional:
  BU_NAME                   defaults to "github-skill-smoke"
  BU_DAEMON_IMPL            defaults to "rust"
  BU_REMOTE_TIMEOUT_MINUTES defaults to "1"
"""

import json
import os
import sys
import time
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO))
os.environ.setdefault("BU_NAME", "github-skill-smoke")

from admin import _browser_use, restart_daemon, start_remote_daemon  # noqa: E402
from helpers import goto, http_get, js, page_info, wait, wait_for_load  # noqa: E402


def poll_browser_status(browser_id, attempts=10, delay=1.0):
    for _ in range(attempts):
        listing = _browser_use("/browsers?pageSize=20&pageNumber=1", "GET")
        item = next((item for item in listing.get("items", []) if item.get("id") == browser_id), None)
        status = item.get("status") if item else "missing"
        if status != "active":
            return status
        time.sleep(delay)
    return status


def main():
    if not os.environ.get("BROWSER_USE_API_KEY"):
        raise SystemExit("BROWSER_USE_API_KEY is required")

    os.environ.setdefault("BU_DAEMON_IMPL", "rust")
    name = os.environ["BU_NAME"]
    timeout = int(os.environ.get("BU_REMOTE_TIMEOUT_MINUTES", "1"))

    browser = None
    result = {
        "name": name,
        "daemon_impl": os.environ["BU_DAEMON_IMPL"],
        "skill": "domain-skills/github/scraping.md",
    }
    try:
        browser = start_remote_daemon(name=name, timeout=timeout)
        result["browser_id"] = browser["id"]

        # Repo metadata workflow from domain-skills/github/scraping.md
        repo = json.loads(http_get("https://api.github.com/repos/browser-use/browser-use"))
        result["repo_api"] = {
            "full_name": repo["full_name"],
            "stargazers_count": repo["stargazers_count"],
            "forks_count": repo["forks_count"],
            "description": repo["description"],
        }
        if result["repo_api"]["full_name"] != "browser-use/browser-use":
            raise RuntimeError("GitHub repo metadata workflow returned the wrong repository")

        # Trending workflow from domain-skills/github/scraping.md
        goto("https://github.com/trending")
        result["loaded"] = wait_for_load()
        wait(2)
        result["after_nav"] = page_info()
        raw_repos = js(
            """
(function(){
  var rows = Array.from(document.querySelectorAll('article.Box-row'));
  return JSON.stringify(rows.map(function(el){
    var h2link = el.querySelector('h2 a');
    var starLink = el.querySelector('a[href*="/stargazers"]');
    var forkLink = el.querySelector('a[href*="/forks"]');
    var langEl = el.querySelector('[itemprop="programmingLanguage"]');
    var todayEl = el.querySelector('.d-inline-block.float-sm-right');
    var descEl = el.querySelector('p');
    return {
      name: h2link ? h2link.innerText.trim().replace(/\\s+/g,' ') : null,
      url: h2link ? 'https://github.com' + h2link.getAttribute('href') : null,
      stars_total: starLink ? starLink.innerText.trim() : null,
      stars_period: todayEl ? todayEl.innerText.trim() : null,
      forks: forkLink ? forkLink.innerText.trim() : null,
      language: langEl ? langEl.innerText.trim() : null,
      desc: descEl ? descEl.innerText.trim() : null
    };
  }));
})()
"""
        )
        repos = json.loads(raw_repos)
        result["trending_count"] = len(repos)
        result["trending_sample"] = repos[:3]

        if len(repos) < 5:
            raise RuntimeError(f"GitHub trending workflow returned too few rows: {len(repos)}")
        first = repos[0]
        if not first.get("name") or not first.get("url", "").startswith("https://github.com/"):
            raise RuntimeError("GitHub trending workflow returned malformed repo data")
        if "github.com/trending" not in result["after_nav"].get("url", ""):
            raise RuntimeError("GitHub trending workflow did not remain on the expected page")
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
