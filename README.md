# bu

The simplest, most powerful browser agent harness: connect to a real browser, keep setup tiny, and let the agent do the rest.

## Setup

Humans should not learn a CLI here; the normal setup is to paste one prompt into Claude Code or Codex and let the agent install and use the repo.

Example prompt:

```text
Clone https://github.com/browser-use/harnessless, install it, enable Chrome remote debugging if needed, and use this browser harness to do the task in my real browser.
```

If you are installing it manually, the only real setup is:

```bash
uv sync
```

Then enable Chrome remote debugging at `chrome://inspect/#remote-debugging`.

## Example task

```text
Use this browser harness to open my real Chrome and submit the form on example.com.
```

## How It Works

`run.py` executes plain Python with `helpers.py` preloaded, and `daemon.py` keeps the CDP websocket and socket bridge alive.

Everything else lives in `SKILL.md`, `interaction-skills/`, and `domain-skills/`.
