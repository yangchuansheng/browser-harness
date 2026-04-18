# bu

The simplest, thinnest, and most powerful browser agent harness.

## Setup

Paste this into Claude Code or Codex:

```text
Clone https://github.com/browser-use/harnessless, set it up for me, enable Chrome remote debugging if needed, read SKILL.md, and use this harness in my real browser for the task I give you next.
```

## Example task

```text
Use this browser harness to open my real Chrome and submit the form on example.com.
```

## Get inspiration

See [domain-skills/](domain-skills/) for examples on other websites.

## How It Works

- `SKILL.md` explains how the harness should be used.
- `run.py` executes plain Python with helpers preloaded.
- `helpers.py` holds the primitives the agent actually calls.
- `daemon.py` keeps the CDP websocket and socket bridge alive.

## Optional: Remote browsers

Useful for sub-agents or deployment.

- Get a Browser Use API key: [cloud.browser-use.com/new-api-key](https://cloud.browser-use.com/new-api-key)
- If the agent needs signup or setup context, point it at [docs.browser-use.com/llms.txt](https://docs.browser-use.com/llms.txt)
