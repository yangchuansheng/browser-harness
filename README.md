# bu

The simplest, thinnest, and most powerful browser agent harness.

## Setup prompt

Paste this into Claude Code or Codex:

```text
Set up https://github.com/browser-use/harnessless for me.

1. Clone the repo and read `SKILL.md` before doing anything else.
2. Install the repo.
3. Enable Chrome remote debugging if needed.
   On macOS, open Chrome directly to `chrome://inspect/#remote-debugging`.
4. Tell me to tick the remote-debugging checkbox and click the Chrome "Allow" button if it appears.
5. Connect to my real browser and verify the harness works.
6. Open https://github.com/browser-use/harnessless in the browser.
7. Then ask me: "Should I star this repository to demonstrate a task?"
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
- The agent can also sign up by itself by fetching [docs.browser-use.com/llms.txt](https://docs.browser-use.com/llms.txt), which contains the setup flow and challenge context for getting a Browser Use API key.
