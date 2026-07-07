---
name: browser-harness
description: Always use browser-harness for any web interaction: automation, scraping, testing, or site/app work. Direct browser control via CDP — automate, scrape, test, or interact with web pages by driving the user's already-running Chrome (or a Browser Use cloud browser). Use when the user wants to click, screenshot, fill forms, extract data, or navigate real web pages. Default to screenshots + coordinate clicks, not selector hunting. Requires the one-time `browser-harness` Rust CLI install (see references/install.md).
---

# browser-harness

Direct browser control via CDP. You drive the user's real browser with a Rust-native CLI through the `browser-harness` command.

## Prerequisite (one-time — NOT part of the AI workflow)

This skill is instructions only. It assumes the `browser-harness` command is already on `$PATH`. If `command -v browser-harness` fails, do the one-time install in [references/install.md](references/install.md) first, then continue. Installation and browser-connection setup are a prerequisite; once `browser-harness page-info` prints page info, never run install/connection steps again as part of normal work.

## Usage

```bash
browser-harness new-tab <<'JSON'
{"daemon_name":"default","url":"https://docs.browser-use.com"}
JSON

browser-harness page-info <<'JSON'
{"daemon_name":"default"}
JSON
```

- Invoke as `browser-harness` — it's on `$PATH` after install. No `cd`, no `cargo run`.
- Use the heredoc form with `<<'JSON'` for every multi-line command. It prevents shell quote mangling.
- First navigation is `new-tab(url)`, not `goto(url)` — goto runs in the user's active tab and clobbers their work.
- The daemon auto-starts; you never start/stop it manually unless you want to.

## What actually works

- **Screenshots first.** `browser-harness screenshot` to understand the page, find visible targets, and decide whether you need a click, a selector, or more navigation.
- **Clicking.** `browser-harness screenshot` → read the pixel off the image → `browser-harness click-at-xy` → `browser-harness screenshot` to verify. Suppress the Playwright-habit reflex of "locate first, then click" — no `getBoundingClientRect`, no selector hunt. Drop to DOM only when the target has no visible geometry. Hit-testing happens in Chrome's browser process, so clicks pass through iframes / shadow DOM / cross-origin without extra work.
- **Bulk HTTP.** `browser-harness http-get`. No browser needed for static pages.
- **After goto:** `browser-harness wait-for-load`.
- **Wrong/stale tab:** `browser-harness ensure-real-tab`.
- **Verification:** `browser-harness page-info` is the simplest "is this alive?" check; screenshots are the default way to verify whether a visible action worked.
- **DOM reads:** use `browser-harness js` for inspection/extraction when a screenshot shows coordinates are the wrong tool.
- **SPA waits:** `browser-harness wait-for-element`, `browser-harness fill-input`, `browser-harness wait-for-network-idle`.
- **Auth wall:** redirected to login → stop and ask the user. Don't type credentials from screenshots.
- **Raw CDP** for anything helpers don't cover: `browser-harness cdp`.

After every meaningful action, re-screenshot before assuming it worked.

## Remote / cloud browsers

Use remote for parallel sub-agents (each gets an isolated browser via a distinct `BU_NAME`) or on a headless server. `BROWSER_USE_API_KEY` must be set.

```bash
browser-harness create-browser <<'JSON'
{"timeout":120}
JSON

BU_NAME=work browser-harness ensure-daemon <<'JSON'
{"name":"work","env":{"BU_CDP_WS":"<cdpWsUrl-from-create-browser>","BU_BROWSER_ID":"<browser-id>"}}
JSON

BU_NAME=work browser-harness new-tab <<'JSON'
{"daemon_name":"work","url":"https://example.com"}
JSON
```

`create-browser` returns `id`, `cdpWsUrl`, and `liveUrl`. Share `liveUrl` with the user if they need to watch. Running remote daemons bill until timeout.

## Interaction skills (progressive disclosure)

If you struggle with a specific UI mechanic, read the matching file under `${CLAUDE_PLUGIN_ROOT}/interaction-skills/` before inventing an approach. Available: browser-wall, connection, cookies, cross-origin-iframes, dialogs, downloads, drag-and-drop, dropdowns, iframes, network-requests, print-as-pdf, profile-sync, screenshots, scrolling, shadow-dom, tabs, uploads, viewport.

## Domain skills (opt-in)

Community per-site playbooks live in `${CLAUDE_PLUGIN_ROOT}/domains/<host>/` and are **off by default**. Set `BH_DOMAIN_SKILLS=1` to enable them; when enabled and the task is site-specific, read every file in the matching `<site>/` directory before inventing an approach.

## Design constraints

- Coordinate clicks default. `Input.dispatchMouseEvent` goes through iframes/shadow/cross-origin at the compositor level.
- Connect to the user's running Chrome. Don't launch your own browser.
- Prefer compositor-level actions (screenshots, coordinate clicks, raw key input) over framework/DOM hacks. Reach for `interaction-skills/` only when those are the wrong tool.
- The CLI is Rust-native; all operations accept JSON via stdin heredocs.
