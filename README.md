<img src="https://r2.browser-use.com/github/ajsdlasnnalsgasld.png" alt="Browser Harness" width="100%" />

# Browser Harness ♞

The simplest, thinnest, **self-healing** harness that gives LLM **complete freedom** to complete any browser task. Built directly on CDP.

The agent writes what's missing, mid-task. No framework, no recipes, no rails. One websocket to Chrome, nothing between.

```
  ● agent: wants to upload a file
  │
  ● browser-harness / bhrun already exposes upload-file
  │
  ● if a capability is missing, the agent extends the harness mid-task
  │   in Rust
  ✓ task completed
```

**You will never use the browser again.**

## Status

Browser Harness is now Rust-native:

- Rust owns the daemon/runtime/control plane
- installed binaries are now Rust-only
- the default installed command is `browser-harness`
- the repo-local fallback is `cargo run --quiet --bin browser-harness -- ...`
- the rewrite/migration work is complete; remaining work is normal product work

## Quick Start

Install once, then use the Rust-native CLI directly:

```bash
cargo run --quiet --manifest-path rust/Cargo.toml --bin browser-harness -- install
export PATH="$HOME/.cargo/bin:$PATH"
browser-harness ensure-daemon
browser-harness page-info <<'JSON'
{"daemon_name":"default"}
JSON
browser-harness new-tab <<'JSON'
{"daemon_name":"default","url":"https://example.com"}
JSON
```

The installer builds the Rust binaries from this checkout and installs them into
`$CARGO_HOME/bin` or `$HOME/.cargo/bin` by default. Re-run the same install
command after pulling new changes if you want to refresh the global binaries.

If you are working inside the repo and have not installed the global command
yet, use:

```bash
cd rust
cargo run --quiet --bin browser-harness -- --help
```

## Setup prompt

Paste into Claude Code or Codex:

```text
Set up https://github.com/browser-use/browser-harness for me.

Read `install.md` first to install and connect this repo to my real browser. Then read `SKILL.md` for normal usage. Prefer the Rust-native CLI path first. When you open a setup or verification tab, activate it so I can see the active browser tab. After it is installed, open this repository in my browser and, if I am logged in to GitHub, ask me whether you should star it for me as a quick demo that the interaction works — only click the star if I say yes. If I am not logged in, just go to browser-use.com.
```

When this page appears, tick the checkbox so the agent can connect to your browser:

<img src="docs/setup-remote-debugging.png" alt="Remote debugging setup" width="520" style="border-radius: 12px;" />

See [domain-skills/](domain-skills/) for example tasks.

## Free remote browsers

Useful for sub-agents or deployment. **Free tier: 3 concurrent browsers, no card required.**

- Grab a key at [cloud.browser-use.com/new-api-key](https://cloud.browser-use.com/new-api-key)
- Or let the agent sign up itself via [docs.browser-use.com/llms.txt](https://docs.browser-use.com/llms.txt) (setup flow + challenge context included).

## Runtime Shape

- `install.md` — first-time install and browser bootstrap
- `SKILL.md` — day-to-day usage
- `rust/bins/browser-harness-cli` — Rust-native top-level CLI facade
- `rust/bins/bhctl` — admin/control plane
- `rust/bins/bhrun` — typed browser operations, waits, and guest runner
- `rust/bins/bhd` — daemon/runtime core

## Legacy Compatibility

The active repo workflow is now Rust-native:

- `browser-harness` — top-level CLI facade
- `bhctl` — admin/control plane
- `bhrun` — typed browser operations and guest runner
- `bhsmoke` — repo-owned smoke coverage

The old repo-local Python shims were moved to `archive/python-legacy/` for
historical reference only.

If you intentionally want Python around the Rust CLI, use the direct
`subprocess` helpers in [docs/python-cli-helpers.md](docs/python-cli-helpers.md)
instead of reviving the archived shim files.

Current policy:

- installed packages no longer ship any Python entrypoint
- the active source tree ships no repo-local Python shim
- archived legacy Python files live only under `archive/python-legacy/`

## Contributing

PRs and improvements welcome. The best way to help: **contribute a new domain skill** under [domain-skills/](domain-skills/) for a site or task you use often (LinkedIn outreach, ordering on Amazon, filing expenses, etc.). Each skill teaches the agent the selectors, flows, and edge cases it would otherwise have to rediscover.

- **Skills are written by the harness, not by you.** Just run your task with the agent — when it figures something non-obvious out, it files the skill itself (see [SKILL.md](SKILL.md)). Please don't hand-author skill files; agent-generated ones reflect what actually works in the browser.
- Open a PR with the generated `domain-skills/<site>/` folder — small and focused is great.
- Bug fixes, docs tweaks, and helper improvements are equally welcome.
- Browse existing skills (`github/`, `linkedin/`, `amazon/`, ...) to see the shape.

If you're not sure where to start, open an issue and we'll point you somewhere useful.

---

[Bitter lesson](https://browser-use.com/posts/bitter-lesson-agent-frameworks) · [Skills](https://browser-use.com/posts/web-agents-that-actually-learn)
