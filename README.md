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
  │   in Rust or the legacy shell, depending on the migration stage
  ✓ task completed
```

**You will never use the browser again.**

## Transition Status

Browser Harness is now in a Rust-first transition:

- Rust owns the daemon/runtime/control plane
- the default installed command is now the Rust-native `browser-harness`
- the repo-local fallback is `cargo run --quiet --bin browser-harness -- ...`

## Quick Start

Install once, then use the Rust-native CLI directly:

```bash
uv tool install -e .
browser-harness ensure-daemon
browser-harness page-info <<'JSON'
{"daemon_name":"default"}
JSON
browser-harness new-tab <<'JSON'
{"daemon_name":"default","url":"https://example.com"}
JSON
```

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

The old Python shell still exists, but it is no longer the default path:

- `browser-harness-py` — explicit legacy heredoc shell
- `run.py` + `runner_cli.py` + `admin_cli.py` — compatibility layer behind that shell
- `helpers.py` and `admin.py` — deprecated repo-local compatibility import paths; they are no longer shipped in installed packages

Use it only when you intentionally need old helper-preloaded behavior. New
docs, smokes, and guest work should land on `browser-harness` / `bhrun` first.

Deprecation details:

- `browser-harness-py` now warns on invocation
- `helpers.py` now warns on import in the source tree; use `runner_cli.py` for stable Python helpers
- `admin.py` now warns on import in the source tree; use `admin_cli.py` for Python admin shims
- set `BROWSER_HARNESS_SUPPRESS_PY_DEPRECATION=1` only if you need to suppress
  those warnings in legacy automation

## Contributing

PRs and improvements welcome. The best way to help: **contribute a new domain skill** under [domain-skills/](domain-skills/) for a site or task you use often (LinkedIn outreach, ordering on Amazon, filing expenses, etc.). Each skill teaches the agent the selectors, flows, and edge cases it would otherwise have to rediscover.

- **Skills are written by the harness, not by you.** Just run your task with the agent — when it figures something non-obvious out, it files the skill itself (see [SKILL.md](SKILL.md)). Please don't hand-author skill files; agent-generated ones reflect what actually works in the browser.
- Open a PR with the generated `domain-skills/<site>/` folder — small and focused is great.
- Bug fixes, docs tweaks, and helper improvements are equally welcome.
- Browse existing skills (`github/`, `linkedin/`, `amazon/`, ...) to see the shape.

If you're not sure where to start, open an issue and we'll point you somewhere useful.

---

[Bitter lesson](https://browser-use.com/posts/bitter-lesson-agent-frameworks) · [Skills](https://browser-use.com/posts/web-agents-that-actually-learn)
