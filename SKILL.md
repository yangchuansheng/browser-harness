---
name: browser-harness
description: Direct browser control via CDP. Use when the user wants to automate, scrape, test, or interact with web pages. Connects to the user's already-running Chrome.
allowed-tools: Bash, Read, Edit, Write
---

# browser-harness

Available interaction skills:
- `cookies.md`
- `cross-origin-iframes.md`
- `dialogs.md`
- `downloads.md`
- `drag-and-drop.md`
- `dropdowns.md`
- `iframes.md`
- `network-requests.md`
- `print-as-pdf.md`
- `screenshots.md`
- `scrolling.md`
- `shadow-dom.md`
- `tabs.md`
- `uploads.md`
- `viewport.md`

Available domain skills:
- `tiktok/upload.md`

**Read `helpers.py` first.** The code is the doc.

## Tool call shape

```bash
bh <<'PY'
# any python. helpers pre-imported. daemon auto-starts.
PY
```

`run.py` calls `ensure_daemon()` before `exec` — you never start/stop manually unless you want to.

## Setup

### Best everyday setup

Clone the repo once, then install it as an editable tool so `bh` works from any directory:

```bash
git clone https://github.com/browser-use/harnessless
cd harnessless
uv tool install -e .
command -v bh
```

That keeps the command global while still pointing at the real repo checkout, so when the agent edits `helpers.py` the next `bh` run uses the new code immediately. `browser-harness` is the readable alias for the same command.

Default to this global setup. Use the local-only flow below only for quick testing inside the repo.

### Make it global for the current agent

After the repo is installed, register this repo's `SKILL.md` with the agent you are using:

- **Codex**: add this file as a global skill at `$CODEX_HOME/skills/browser-harness/SKILL.md` (often `~/.codex/skills/browser-harness/SKILL.md`). A symlink to this repo's `SKILL.md` is fine.
- **Claude Code**: add an import to `~/.claude/CLAUDE.md` that points at this repo's `SKILL.md`, for example `@~/src/harnessless/SKILL.md`.

That makes new Codex or Claude Code sessions in other folders load the browser harness instructions automatically.

To confirm the memory/instructions are loaded:

- **Codex**: start a new session and check that `browser-harness` appears in the available skills.
- **Claude Code**: run `/memory` and confirm that `~/.claude/CLAUDE.md` and its import are listed.

### Simplest local setup

1. Run `uv sync`.
2. First try the harness directly. If this works, skip setup entirely:

```bash
uv run bh <<'PY'
ensure_real_tab()
print(page_info())
PY
```

3. If that fails and Chrome is already running, open `chrome://inspect/#remote-debugging` in the existing Chrome profile instead of launching a fresh Chrome process.
   On macOS:

```bash
osascript -e 'tell application "Google Chrome" to activate' \
          -e 'tell application "Google Chrome" to open location "chrome://inspect/#remote-debugging"'
```

   On Linux: use the already-running Chrome window and open that URL manually.
4. If Chrome is not running, start Chrome first and let the user choose their normal profile if Chrome opens the profile picker. Only after that, open `chrome://inspect/#remote-debugging`.
   On macOS: `open -a "Google Chrome"`
5. Tell the user to tick the remote-debugging checkbox. If Chrome shows `Allow`, tell the user to click it once.
6. Do not ask the user to say "continue". Poll every few seconds and retry the same connect attempt once the permission flow finishes.
7. If setup still lands on the profile picker, have the user choose their normal profile, then open `chrome://inspect/#remote-debugging` in that profile and keep polling instead of restarting the explanation.
8. Verify with:

```bash
uv run bh <<'PY'
ensure_real_tab()
if not current_tab()["url"] or current_tab()["url"].startswith(INTERNAL):
    new_tab("about:blank")
print(page_info())
PY
```

If that fails with a stale websocket or stale socket, restart the daemon once and retry:

Run this from the repo root:

```bash
uv run python - <<'PY'
from helpers import kill_daemon
kill_daemon()
PY
```

### Remote browsers

Remote is optional. Use it for parallel agents, sub-agents, or deployment.

If `BROWSER_USE_API_KEY` is already present in `.env` or the environment, start a remote daemon with:

Run this from the repo root:

```bash
uv run python - <<'PY'
from helpers import start_remote_daemon
print(start_remote_daemon("work"))
PY
BU_NAME=work uv run bh <<'PY'
print(page_info())
PY
```

Leaving a remote daemon running bills until the session timeout.

Parallel agents should use distinct `BU_NAME`s and can share the same `helpers.py`; shared improvements are expected, and changes should stay general enough that other agents benefit rather than break.

## Search first

After cloning the repo, search `interaction-skills/` for reusable UI mechanics and `domain-skills/` for site-specific workflows before inventing a new approach.

Useful commands:

```bash
rg --files interaction-skills domain-skills
rg -n "dropdown|iframe|upload|tiktok" interaction-skills domain-skills
```

## Post-task ritual (self-improving harness)

After every browser task, extract ONE generalizable friction point from the interaction (a failed selector strategy, a slow pattern, a missing helper, a confusing result) and make the **simplest possible** improvement:
- a 2-line helper in `helpers.py`, OR
- a one-line gotcha in this file, OR
- a correction to a wrong recipe here.

Commit with the task. The skill gets sharper every use. Skip only if nothing was surprising.

If you solve a specific website and learn a lot, create a PR to this repo with reusable learnings in `domain-skills/` or `interaction-skills/` — no secrets, no user data, no overfit recipes, just how the site works, what to wait for, and what patterns matter.

## What actually works

- **Scraping**: `js("...custom query...")`. Bespoke selectors beat generic DOM helpers.
- **Clicking**: `screenshot()` → look → `click(x, y)`. Passes through iframes/shadow/cross-origin at the compositor level.
- **Bulk HTTP**: `http_get(url)` + `ThreadPoolExecutor`. No browser for static pages (249 Netflix pages in 2.8s).
- **After goto**: `wait_for_load()`.
- **Wrong/stale tab**: `ensure_real_tab()`. Daemon also auto-recovers from stale sessions on next call.
- **Verification**: `ensure_real_tab(); print(page_info())` is the simplest "is this alive?" check.
- **Iframe sites** (Azure blades, Salesforce): `click(x, y)` passes through; for DOM use `js(expr, target_id=iframe_target("sandbox"))`. Iframe rects are iframe-local — add the host iframe's offset for page coords.
- **Auth wall**: redirected to login → stop and ask the user. Don't type credentials from screenshots.
- **Raw CDP** for anything helpers don't cover: `cdp("Domain.method", **params)`.

## Design constraints

- **Coordinate clicks default.** `Input.dispatchMouseEvent` goes through iframes/shadow/cross-origin at the compositor level.
- **Connect to the user's running Chrome.** Don't launch your own browser.
- **`cdp-use` is only for `CDPClient.send_raw`.** Prefer raw CDP strings over typed wrappers.
- **`run.py` stays tiny.** No argparse, subcommands, or extra control layer.
- **Helpers stay short.** No classes, no extra deps beyond stdlib + `cdp-use` + `websockets`.
- **Don't add a manager layer.** No retries framework, session manager, daemon supervisor, config system, or logging framework.

## Architecture

```text
Chrome / Browser Use cloud -> CDP WS -> daemon.py -> /tmp/bu-<NAME>.sock -> run.py
```

- Protocol is one JSON line each way.
- Requests are `{method, params, session_id}` for CDP or `{meta: ...}` for daemon control.
- Responses are `{result}` / `{error}` / `{events}` / `{session_id}`.
- `BU_NAME` namespaces socket, pid, and log files.
- `BU_CDP_WS` overrides local Chrome discovery for remote browsers.
- `BU_BROWSER_ID` + `BROWSER_USE_API_KEY` lets the daemon stop a Browser Use cloud browser on shutdown.

## Gotchas (field-tested)

- **Chrome 144+ `chrome://inspect/#remote-debugging` does NOT serve `/json/version`.** Read `DevToolsActivePort` instead.
- **Try attaching before asking for setup.** If `uv run bh` already works, skip the remote-debugging instructions entirely.
- **The first connect may block on Chrome's Allow dialog.** If setup hangs, ask the user to click Allow, then retry once.
- **Chrome may open the profile picker before any real tab exists.** Pick the user's normal profile first; remote debugging can be enabled while the browser is still not in a usable signed-in context.
- **On macOS, if Chrome is already running, prefer AppleScript `open location` over `open -a ... URL`.** It reuses the current profile and avoids creating an extra startup path through the profile picker.
- **Omnibox popups are fake `page` targets.** Filter `chrome://omnibox-popup...` and other internals when you need a real tab.
- **CDP target order != Chrome's visible tab-strip order.** Use UI automation when the user means "the first/second tab I can see"; `Target.activateTarget` only shows a known target.
- **Default daemon sessions can go stale.** `ensure_real_tab()` re-attaches to a real page.
- **`no close frame received or sent` usually means a stale daemon / websocket.** Kill the daemon once and retry before assuming setup is wrong.
- **Keep the two `INTERNAL` tuples in sync.** `daemon.py` and `helpers.py` each define one.
- **Browser Use API is camelCase on the wire.** `cdpUrl`, `proxyCountryCode`, etc.
- **Remote `cdpUrl` is HTTPS, not ws.** Resolve the websocket URL via `/json/version`.
- **Stop cloud browsers with `PATCH /browsers/{id}` + `{\"action\":\"stop\"}`.**
- **React / controlled inputs ignore `el.value=...`.** Use the native setter to make React see the change:
  `Object.getOwnPropertyDescriptor(HTMLInputElement.prototype,'value').set.call(el,v); el.dispatchEvent(new Event('input',{bubbles:true}))`.
- **Radio/checkbox via React**: prefer `el.click()` over `el.checked=true` — React listens to the click event to drive state.
- **UI-library buttons (MUI Select, dropdown overlays)**: JS `.click()` on `[role=button]` often does NOT fire the library's handler. Screenshot → `click(x,y)` via CDP instead.
- **Keyboard listeners checking `e.key==='Enter'` on `keypress`**: CDP's `char` event doesn't always fire DOM `keypress` for special keys. Use `dispatch_key(selector, 'Enter')`.
- **`alert()`/`confirm()` block the page thread.** Call `capture_dialogs()` BEFORE the action, read via `dialogs()` after.
- **Same-origin nested iframes** don't show up as CDP targets — walk `document.querySelector('iframe').contentDocument` (or `contentWindow`) recursively. Cross-origin iframes DO appear as targets; use `iframe_target("...")`.
- **Shadow DOM**: `document.querySelector` doesn't pierce shadow roots. Walk via `element.shadowRoot.querySelectorAll` (and recurse).
- **Submitting forms**: the "Submit" button isn't always the first `button[type=submit]` — on React Native Web etc. contact-method buttons share that type. Prefer the button whose text matches `/submit/i`, fall back to `form.requestSubmit()`.
- **Form success signals vary**: visible `#success-message`, captured `alert()` text, console log, or body text change. Check all sources — don't assume one convention.

## Interaction notes

- `interaction-skills/` holds reusable UI mechanics such as dialogs, tabs, dropdowns, iframes, and uploads.
- `domain-skills/` holds site-specific workflows and should be updated when you discover reusable patterns for a website.
