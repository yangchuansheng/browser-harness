# Uploads

Treat file upload as browser state, not DOM text entry.

The Rust-native path is:

- `bhrun upload-file`
- `browser-harness upload-file`
- `bh_guest_sdk::upload_file(...)`
- `scripts._runner_cli.upload_file(...)` for repo-local Python examples

## Preferred Flow

1. Navigate to the target page or iframe.
2. If the input lives inside a cross-origin iframe, resolve it first with
   `iframe_target(...)` and pass that `target_id`.
3. Point `upload-file` at the actual `<input type="file">`, not the visible
   styled button around it.
4. Verify the upload through DOM state, filename text, preview state, or a
   network wait.

## Example

```python
from scripts._runner_cli import upload_file

upload_file("#resume", ["/tmp/resume.pdf"])
```

Iframe-scoped example:

```bash
bhrun upload-file <<'JSON'
{"daemon_name":"default","selector":"input[type=file]","files":["/tmp/resume.pdf"],"target_id":"<iframe-target-id>"}
JSON
```

## What This Solves

Use `upload-file` when:

- the site uses a hidden real file input behind a styled button
- the page expects the browser file picker result, not typed text
- you need the selected file to appear in `input.files`

Do not use it for:

- drag-and-drop file zones that never expose a file input
- normal text fields
- remote-browser cases where the browser process cannot see your local file path

## Verification

Prefer one of these after upload:

- read `input.files.length`
- read `input.files[0].name`
- wait for a preview or filename chip to appear
- wait for the upload request/response

## Local Acceptance

- `scripts/bhrun_upload_smoke.py`

```bash
BU_BROWSER_MODE=local BU_DAEMON_IMPL=rust python3 scripts/bhrun_upload_smoke.py
```

That smoke:

- injects a real `<input type="file">` into `about:blank`
- uploads a local temp file through `bhrun upload-file`
- verifies the page sees the filename and file text through `File.text()`
