# Downloads

Separate browser-triggered downloads from plain HTTP fetches.

The Rust-native path is:

- `bhrun configure-downloads`
- `bhrun wait-for-download`
- `browser-harness configure-downloads`
- `browser-harness wait-for-download`
- `bh_guest_sdk::configure_downloads(...)`
- `bh_guest_sdk::wait_for_download(...)`

## Preferred Flow

1. Use `http_get(...)` when you only need bytes from a URL and do not need the
   browser download flow.
2. Use `configure-downloads` when the page triggers a real browser download.
3. Use `wait-for-download` to prove the browser emitted
   `Browser.downloadWillBegin`.
4. On local browsers, also verify the file appears on disk.

## Example

```bash
bhrun configure-downloads <<'JSON'
{"daemon_name":"default","download_path":"/tmp/bh-downloads"}
JSON

bhrun wait-for-download <<'JSON'
{"daemon_name":"default","filename":"report.csv","timeout_ms":5000,"poll_interval_ms":100}
JSON
```

## What This Proves

`wait-for-download` is the browser-truth signal that a download started.

Use it for:

- export buttons
- blob URL downloads
- links with `download=...`
- pages that stay on the same URL while starting a file download

Do not replace it with only DOM checks or only `page_info()`.

## Local Acceptance

- `scripts/bhrun_download_smoke.py`

```bash
BU_BROWSER_MODE=local BU_DAEMON_IMPL=rust python3 scripts/bhrun_download_smoke.py
```

That smoke:

- configures a temp download directory
- triggers a blob download from the page
- waits for `Browser.downloadWillBegin`
- verifies the final file exists and its content matches the blob payload
