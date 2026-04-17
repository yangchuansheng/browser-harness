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

**First daemon start of a session may hang** on the CDP WebSocket handshake — Chrome shows a native "Allow debugging connections" dialog the user has to click. Be patient (give ~15s), then retry. Don't surface an error on the first timeout.

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

## Gotchas (field-tested)

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
