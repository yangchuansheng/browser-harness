# Browser Harness ♞

Browser Harness is a thin, Rust-native browser runtime for agents. It connects
LLMs to a real browser over CDP without adding a large framework in between.

It exposes stable browser operations, keeps the control plane small, and still
lets the agent extend missing behavior mid-task when it needs to.

`domains/<site>/skill.md` is the knowledge layer: URL patterns, selectors,
waits, traps, APIs, and site-specific workflow notes. Rust guests are optional
packaged workflows, not the default artifact for every domain. Some domain docs
use Python-like helper syntax as pseudocode for harness operations; that does
not mean Python is required to use this repo.

## Acknowledgements

This project is a Rust reimplementation of the original
[`browser-use/browser-harness`](https://github.com/browser-use/browser-harness)
Python harness.

The core harness idea and direction came from that upstream project. Many
thanks to the `browser-use` maintainers for creating and sharing the original
work that this repo builds on.

```
  ● agent: needs a site-specific browser workflow
  │
  ● browser-harness exposes stable primitives
  │
  ● a helper is missing
  │
  ● the agent adds it in Rust or as a WASM guest
  ✓ task completed
```

## What This Repo Is

This repo carries the same core thesis as the original
[`browser-use/browser-harness`](https://github.com/browser-use/browser-harness),
but reimplements the runtime in Rust.

The upstream original keeps the harness extremely small with files like
`run.py`, `helpers.py`, `admin.py`, and `daemon.py`. That design proves the
thesis: agents do not need a huge browser framework to be useful.

This repo keeps that same thesis, but moves the control plane into compiled
Rust binaries and crates:

- `browser-harness` as the top-level CLI
- `bhd` as the daemon/runtime
- `bhrun` as the typed runner and guest executor
- `bhctl` as the admin/control-plane binary

So the right comparison is not "framework vs harness". It is:

- original Browser Harness: tiny Python runtime
- this repo: Rust systems rewrite of that runtime
- `browser-harness-js`: protocol-first JS/TS variant with almost no helpers

## Why Rust

We chose Rust because this project is fundamentally a systems boundary:

- It runs a long-lived daemon that owns a live browser websocket and session state.
- It exposes a stable CLI/runtime surface that other tools can depend on.
- It needs typed protocol boundaries between daemon, runner, admin commands, and guest execution.
- It is moving toward a Rust + WASM guest model, where the host and the guest SDK benefit from sharing a strongly typed language and build toolchain.

In practice, Rust gives this project a few concrete advantages over the original
Python harness:

- single native binaries instead of a Python packaging/runtime story
- stronger typing around protocol payloads and internal boundaries
- easier long-lived daemon behavior and runtime encapsulation
- a cleaner path to capability-gated WASM guests
- one implementation language for CLI, daemon, SDK, and host runtime

This is not "Rust because Rust is cool". It is Rust because this repo is no
longer just a handful of editable helper functions; it is now a durable runtime
surface.

## Comparison

Browser Harness should be read against the two adjacent Browser Use projects:

| Project | Main shape | Best when | Tradeoff |
| --- | --- | --- | --- |
| [`browser-use/browser-harness`](https://github.com/browser-use/browser-harness) | Tiny Python harness with editable helper files and a small daemon bridge | You want the original minimal harness exactly as designed | Smaller and simpler, but less suited to a larger typed runtime surface |
| [`browser-use/browser-harness-js`](https://github.com/browser-use/browser-harness-js) | JS/TS CDP surface with generated protocol wrappers and almost no helpers | You want protocol-level freedom in JS/TS and are happy writing close to raw CDP | Maximum flexibility, but more protocol-shaped and less ergonomic for repeated higher-level workflows |
| [`yangchuansheng/browser-harness`](https://github.com/yangchuansheng/browser-harness) (this repo) | Rust daemon + runner + CLI + guest runtime | You want the harness idea, but as a durable systems runtime with stable operations and a WASM path | Heavier than the original Python harness, less raw than the JS variant |

Another way to frame it:

- the original Python repo is the minimal harness proof
- `browser-harness-js` is the raw protocol-first variant
- this repo is the runtime-first Rust variant

If you want the shortest decision rule:

- Choose the original Python harness if you want the smallest possible editable harness.
- Choose `browser-harness-js` if you want the agent to speak almost-direct CDP in JS/TS.
- Choose this repo if you want the harness idea packaged as a typed runtime, daemon, CLI, and guest host.

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
Set up https://github.com/yangchuansheng/browser-harness.git for me.

Read `install.md` first to install and connect this repo to my real browser. Then read `SKILL.md` for normal usage. Prefer the Rust-native CLI path first. When you open a setup or verification tab, activate it so I can see the active browser tab. After it is installed, open this repository in my browser and, if I am logged in to GitHub, ask me whether you should star it for me as a quick demo that the interaction works — only click the star if I say yes. If I am not logged in, just go to browser-use.com.
```

When this page appears, tick the checkbox so the agent can connect to your browser:

<img src="docs/setup-remote-debugging.png" alt="Remote debugging setup" width="520" style="border-radius: 12px;" />

See [domains/](domains/) for the active site-specific knowledge tree.

## Remote Browsers

If you want to use a remote browser instead of attaching to a local Chrome or
Edge instance, create one at [cloud.browser-use.com](https://cloud.browser-use.com).

That flow gives you the remote browser information needed by the harness, such
as the live browser session and CDP connection details.

## Docs

- [install.md](install.md) — first-time install and browser bootstrap
- [SKILL.md](SKILL.md) — day-to-day operator/agent guide
- [docs/architecture.md](docs/architecture.md) — runtime layout and core components
- [docs/development.md](docs/development.md) — workspace commands and verification
- [docs/wasm-guests.md](docs/wasm-guests.md) — current guest model, capabilities, and build/run flow
- [docs/future-wasm.md](docs/future-wasm.md) — long-term guest direction
- [docs/python-integration.md](docs/python-integration.md) — optional Python subprocess wrappers

## Project Structure

- `rust/` — binaries, crates, guest modules, and workspace metadata
- `domains/` — active site-specific packages, centered on `domains/<site>/skill.md`
- `interaction-skills/` — reusable browser mechanics
- `docs/` — architecture, development, and future design notes
- `scripts/` — repo maintenance helpers such as leak scanning
