# Browser Harness

The simplest, thinnest, and most powerful browser agent harness.

## Setup prompt

Paste this into Claude Code or Codex:

```text
Set up https://github.com/browser-use/harnessless for me.

1. Clone the repo and read `SKILL.md` before doing anything else.
2. Move into the repo folder and run `uv sync`.
3. Enable Chrome remote debugging if needed.
   On macOS, open Chrome directly to `chrome://inspect/#remote-debugging`.
4. Tell me to tick the remote-debugging checkbox and click the Chrome "Allow" button if it appears.
5. Connect to my real browser and verify the harness works.
6. Open https://github.com/browser-use/harnessless in the browser.
7. If I am already signed in to GitHub, star the repository to verify the harness works.
8. If I am not signed in, ask me what task I want to run instead.
```

## Example task

```text
Star this repository.
```

## Get inspiration

See [domain-skills/](domain-skills/) for examples on other websites.

## How It Works

- `SKILL.md` is about 100 lines and explains how the harness should be used.
- `run.py` is about 4 lines and just executes plain Python with helpers preloaded.
- `helpers.py` is about 260 lines and holds the primitives the agent calls and constantly modifies to sharpen its own harness for the task.
- `daemon.py` is about 200 lines and keeps the CDP websocket and socket bridge alive.

## Optional: Remote browsers

Useful for sub-agents or deployment.

- Get a Browser Use API key: [cloud.browser-use.com/new-api-key](https://cloud.browser-use.com/new-api-key)
- The agent can also sign up by itself by fetching [docs.browser-use.com/llms.txt](https://docs.browser-use.com/llms.txt), which contains the setup flow and challenge context for getting a Browser Use API key.
