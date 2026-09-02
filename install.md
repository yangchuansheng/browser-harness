---
name: browser-harness-install
description: Install and bootstrap browser-harness into the current agent, then connect it to the user's real Chrome with minimal prompting.
---

# browser-harness install

Use this file only for first-time install, reconnect, or cold-start browser bootstrap. For day-to-day browser work, read `SKILL.md`.

Rust now owns the runtime/control plane. The repo-local Rust-native entrypoint
for installation is:

```bash
cargo run --quiet --manifest-path rust/Cargo.toml --bin browser-harness -- install
```

The installed Rust-native command is:

```bash
browser-harness --help
```

If the global command is not installed yet, use the repo-local fallback:

```bash
cd rust
cargo run --quiet --bin browser-harness -- --help
```

## Install prompt contract

When you open a setup or verification tab, activate it so the user can actually see the active browser tab.

## Best everyday setup

Clone the repo once into a durable location, then install the Rust binaries so
`browser-harness` works from any directory:

```bash
git clone https://github.com/browser-use/browser-harness
cd browser-harness
cargo run --quiet --manifest-path rust/Cargo.toml --bin browser-harness -- install
export PATH="$HOME/.cargo/bin:$PATH"
browser-harness --help
browser-harness verify-install
```

That installs `browser-harness`, `browser-harness-mcp`, `bhctl`, `bhrun`, and `bhd` into
`$CARGO_HOME/bin` or `$HOME/.cargo/bin` by default. Re-run the same install
command after pulling new changes when you want to refresh the installed
binaries. Prefer a stable checkout path like `~/Developer/browser-harness`, not
`/tmp`.

For the Rust-native repo-local path, use:

```bash
cd rust
cargo run --quiet --bin browser-harness -- --help
```

## Make it global for the current agent

After the repo is installed, register this repo's `SKILL.md` with the agent you are using:

- **Codex**: add this file as a global skill at `$CODEX_HOME/skills/browser-harness/SKILL.md` (often `~/.codex/skills/browser-harness/SKILL.md`). A symlink to this repo's `SKILL.md` is fine.
- **Claude Code**: add an import to `~/.claude/CLAUDE.md` that points at this repo's `SKILL.md`, for example `@~/src/browser-harness/SKILL.md`.

Codex command:

```bash
mkdir -p "${CODEX_HOME:-$HOME/.codex}/skills/browser-harness" && ln -sf "$PWD/SKILL.md" "${CODEX_HOME:-$HOME/.codex}/skills/browser-harness/SKILL.md"
```

That makes new Codex or Claude Code sessions in other folders load the runtime browser harness instructions automatically. An empty `~/.codex/skills/browser-harness/` directory is fine; the symlink command above populates it.

## Browser connection reference

Browser Harness can attach to Browser Use cloud browsers or to a local
Chrome/Chromium-family browser.

**Cloud browsers.** Authenticate once with `browser-harness auth login --api-key-stdin`
or export `BROWSER_USE_API_KEY`; `browser-harness auth status` reports whether
the key came from the environment or the stored auth file, and
`browser-harness auth logout` removes the stored key. Use
`browser-harness create-browser`; pass the returned `cdpWsUrl` to a daemon
through `BU_CDP_WS`, and keep `BU_BROWSER_ID` so shutdown stops billing and
persists the cloud profile state. Profile sync commands live in
`interaction-skills/profile-sync.md` and sync cookies only.

For a machine-readable named-daemon health check, run:

```bash
browser-harness doctor remote
```

The versioned JSON report checks the existing daemon socket and leaves browser
discovery and daemon startup untouched.

**Local Way 1 — real profile.** In the running browser, open
`chrome://inspect/#remote-debugging` and tick "Allow remote debugging for this
browser instance". The setting is per-profile and sticky. This path uses the
user's real logins, extensions, history, and tabs.

On macOS, Chrome 144+ may show an "Allow remote debugging?" sheet for each new
connection. Handle it without bringing Chrome to the foreground:

```bash
browser-harness mac-approve
```

Continue browser work when the helper returns `ready`; each other status
includes the required action. The initial checkbox remains a one-time manual
Chrome setup step because the harness only gains CDP access after it is enabled.
The helper requires Accessibility permission for the app launching the CLI,
such as Terminal, iTerm, Codex, or an IDE, in System Settings > Privacy &
Security > Accessibility.

**Local Way 2 — isolated automation profile.** Launch Chrome/Chromium with a
non-default user-data-dir and a debugging port, then point the harness at it:

```bash
/Applications/Google\ Chrome.app/Contents/MacOS/Google\ Chrome \
  --remote-debugging-port=9222 \
  --user-data-dir="$HOME/.browser-harness/chrome-9222"
export BU_CDP_URL="http://127.0.0.1:9222"
browser-harness page-info <<'JSON'
{"daemon_name":"default"}
JSON
```

The `--user-data-dir` must not be Chrome's platform default. Chrome 136+ ignores
remote-debugging flags against the default profile directory, and copied default
profile cookies generally do not survive because their encryption key is bound
to the original directory. Use Way 1 for real logins; use Way 2 or cloud when
unattended automation must avoid popups.

`BU_CDP_WS` always wins when already set. `BU_CDP_URL` resolves `/json/version`
and falls back to `DevToolsActivePort` when newer Chrome builds return 404.
`BH_CONFIG_DIR` overrides the default config directory for stored auth; `BH_HOME`
or `BROWSER_HARNESS_HOME` can point all Browser Harness config at a custom root.
`BH_RUNTIME_DIR` keeps socket/pid files in a short runtime path; `BH_TMP_DIR`
keeps logs, screenshots, and debug artifacts in a separate temp path. For Snap
or Flatpak browser troubleshooting, see [docs/snap-linux-headless.md](docs/snap-linux-headless.md).

## Browser bootstrap

1. Install the Rust binaries if `browser-harness` is still missing:

```bash
command -v browser-harness >/dev/null || cargo run --quiet --manifest-path rust/Cargo.toml --bin browser-harness -- install
command -v browser-harness >/dev/null || export PATH="$HOME/.cargo/bin:$PATH"
browser-harness --help
```

   If the command is still missing after that, the current shell probably does
   not have `$CARGO_HOME/bin` or `$HOME/.cargo/bin` on `PATH`; prepend it and
   retry.
2. First try the Rust-native harness directly. If this works, skip manual browser setup:

```bash
browser-harness page-info <<'JSON'
{"daemon_name":"default"}
JSON
```

   Reuse an existing healthy daemon if it is already responding. Do not kill it during setup unless the attach is clearly stale and you are confident no other agent is using the same `BU_NAME`. For parallel agents, use distinct `BU_NAME`s so they do not fight over the same default session.

3. If it failed, **read the error and escalate from there — do not assume you need `chrome://inspect`**. The remote-debugging checkbox is per-profile sticky in Chrome, so any profile that has had it toggled on once will auto-enable CDP on every future launch; the inspect page is only needed the first time per profile.

   - **No Chrome process running** → just start Chrome and re-run the harness. On macOS: `open -a "Google Chrome"`. Do *not* navigate to `chrome://inspect` yet — if the user has ever ticked the checkbox on this profile, the harness will attach on its own.
   - **`DevToolsActivePort` missing or empty after Chrome is up** → remote-debugging has never been enabled on this profile. *This* is when you open `chrome://inspect/#remote-debugging` and ask the user to tick the checkbox. Once ticked, the setting sticks.
   - **Port present but `connection refused` / `DevTools not live yet` / `/json/version` 404** → Chrome is mid-startup. Just keep polling for up to 30 seconds; do not restart Chrome and do not open the inspect page.
   - **`no close frame received or sent` / stale websocket** → the daemon (not Chrome) is the problem. Run `restart_daemon()` once and retry — see step 7 below.

   When you do need to open the inspect page on macOS and Chrome is already running, prefer AppleScript so it reuses the current profile instead of going through the picker:

```bash
osascript -e 'tell application "Google Chrome" to activate' \
          -e 'tell application "Google Chrome" to open location "chrome://inspect/#remote-debugging"'
```

   On Linux: open that URL manually in the existing Chrome window.
   If Chrome shows the profile picker first, tell the user to choose their normal profile, *then* (only if `DevToolsActivePort` is still missing) open the inspect page in that profile. Keep polling instead of waiting for the user to type a follow-up.
4. Be explicit with the user about the manual Chrome actions: choose their normal profile if the profile picker is open, and tick the checkbox in the remote-debugging tab. On macOS, run `browser-harness mac-approve` for the per-connection sheet.
5. Try to do everything yourself. Ask the user for the profile selection or initial checkbox when needed. Run `browser-harness mac-approve` yourself on macOS, continue when it returns `ready`, and follow any other printed status. Keep retrying startup for at least 30 seconds through connection-refused, stale websocket, and similar transient attach errors.
6. If setup still lands on the profile picker, have the user choose their normal profile, then (only if `DevToolsActivePort` is still missing) open `chrome://inspect/#remote-debugging` in that profile and keep polling instead of restarting the explanation. As soon as attach succeeds, continue immediately with the verification task without asking again.
7. Verify with:

```bash
browser-harness goto <<'JSON'
{"daemon_name":"default","url":"https://github.com/browser-use/browser-harness"}
JSON
browser-harness wait-for-load <<'JSON'
{"daemon_name":"default","timeout":15.0}
JSON
browser-harness page-info <<'JSON'
{"daemon_name":"default"}
JSON
```

If that fails with a stale websocket or stale socket, restart the daemon once and retry:

```bash
browser-harness restart-daemon
```

8. After install and browser bootstrap succeeds, navigate to `https://github.com/browser-use/browser-harness` so the user can see the harness has attached to their browser, then continue with the user's requested browser task.

## Recording Consent

Run `browser-harness recordings`. If it reports `(default)`, ask the user once:

> Enable local browser recordings? This saves screenshots and action traces on
> this machine, which may include sensitive page content, so you can later ask
> "show me what you did" or request a video. Videos are never generated
> automatically. [y/N]

Default to no. Run `browser-harness recordings enable` only after yes; otherwise
run `browser-harness recordings disable`. Preserve an existing `(config)` or
`(BH_RECORD)` preference during upgrades instead of asking again.

> **Rust fork note:** The recording engine is not yet ported to Rust.
> `browser-harness recordings` subcommand is not available in the Rust CLI;
> recording consent applies to the upstream Python harness only.

## Cold-start reminders

- Try attaching before asking the user to change anything. Decide what to escalate based on the harness's error message, not on whether Chrome is visibly running.
- The remote-debugging checkbox is per-profile sticky in Chrome. If it has ever been ticked on a profile, just launching Chrome is enough — only navigate to `chrome://inspect/#remote-debugging` when `DevToolsActivePort` is genuinely missing.
- On macOS, the first connect may block on Chrome's `Allow` sheet; run `browser-harness mac-approve`. Chrome may also stop first on the profile picker.
- `DevToolsActivePort` can exist before the port is actually listening. Treat connection refused as "still enabling" and keep polling briefly.
- If the port is listening but `/json/version` returns `404`, treat that as expected on newer Chrome builds and retry `browser-harness`.
- Chrome may open the profile picker before any real tab exists.
- On macOS, prefer AppleScript `open location` over `open -a ... URL` when Chrome is already running.
- Microsoft Edge (including Beta/Dev/Canary) works too — substitute the app name; steps are identical.

## Optional Python Integration

Installed binaries are Rust-only.

If you intentionally want to orchestrate `browser-harness` from Python, keep
that logic outside the repo runtime layer and use the thin `subprocess`
pattern in [docs/python-integration.md](docs/python-integration.md).
