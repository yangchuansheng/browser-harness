---
name: harnesless
description: Direct browser control via CDP. Use when the user wants to automate, scrape, test, or interact with web pages. Connects to the user's already-running Chrome.
allowed-tools: Bash, Read, Edit, Write
---

# Harnesless

Project: `/Users/greg/Documents/browser-use/hackathons/harnesless/`

**Read `helpers.py` first.** ~150 lines, one tool call. The code is the doc.

## Tool call shape

```bash
cd /Users/greg/Documents/browser-use/hackathons/harnesless && uv run run.py <<'PY'
# any python. helpers pre-imported. daemon auto-starts.
PY
```

`run.py` calls `ensure_daemon()` before `exec` — you never start/stop manually unless you want to.

## Setup (once per machine)

`uv sync`. User enables `chrome://inspect/#remote-debugging` in their Chrome.

## Daemon management

- Socket at `/tmp/harnesless.sock` IS the lock.
- `ensure_daemon()` — idempotent, starts if not alive.
- `kill_daemon()` — graceful shutdown + pkill fallback.
- Starting the daemon twice is safe: the second one exits on seeing the socket.
- Logs: `/tmp/harnesless.log`.

## Post-task ritual (self-improving harness)

After every browser task, extract ONE generalizable friction point from the interaction (a failed selector strategy, a slow pattern, a missing helper, a confusing result) and make the **simplest possible** improvement:
- a 2-line helper in `helpers.py`, OR
- a one-line gotcha in `AGENTS.md`, OR
- a correction to a wrong recipe here.

Commit with the task. The skill gets sharper every use. Skip only if nothing was surprising.

## What actually works

- **Scraping**: `js("...custom query...")`. Bespoke selectors beat generic DOM helpers.
- **Clicking**: `screenshot()` → look → `click(x, y)`. Passes through iframes/shadow/cross-origin at the compositor level.
- **Bulk HTTP**: `http_get(url)` + `ThreadPoolExecutor`. No browser for static pages (249 Netflix pages in 2.8s).
- **After goto**: `wait_for_load()`.
- **Wrong/stale tab**: `ensure_real_tab()`. Daemon also auto-recovers from stale sessions on next call.
- **Iframe sites** (Azure blades, Salesforce): `click(x, y)` passes through; for DOM use `js(expr, target_id=iframe_target("sandbox"))`. Iframe rects are iframe-local — add the host iframe's offset for page coords.
- **Auth wall**: redirected to login → stop and ask the user. Don't type credentials from screenshots.
- **Raw CDP** for anything helpers don't cover: `cdp("Domain.method", **params)`.
