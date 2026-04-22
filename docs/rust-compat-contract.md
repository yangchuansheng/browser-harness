# Rust Compatibility Contract

This document defines the Python-facing contract that the in-repo Rust rewrite
must preserve while `admin_cli.py` and `helpers.py` remain the compatibility
shell. `admin.py` is only a compatibility alias for `admin_cli.py`.

## Runtime Files

- Daemon name: `BU_NAME`, default `default`
- Unix socket: `/tmp/bu-<name>.sock`
- PID file: `/tmp/bu-<name>.pid`
- Log file: `/tmp/bu-<name>.log`

`admin_cli.py`, `helpers.py`, `bhd`, and `bhctl` must agree on these paths.

## Daemon Socket Protocol

Transport:

- Unix domain socket at `/tmp/bu-<name>.sock`
- One JSON object per line
- One JSON response per request

Request shapes:

```json
{"meta":"drain_events"}
{"meta":"session"}
{"meta":"set_session","session_id":"..."}
{"meta":"pending_dialog"}
{"meta":"page_info"}
{"meta":"current_tab"}
{"meta":"list_tabs","params":{"include_internal":false}}
{"meta":"switch_tab","params":{"target_id":"..."}}
{"meta":"new_tab","params":{"url":"https://example.com"}}
{"meta":"ensure_real_tab"}
{"meta":"iframe_target","params":{"url_substr":"frames.example.test"}}
{"meta":"wait_for_load","params":{"timeout":15.0}}
{"meta":"goto","params":{"url":"https://example.com"}}
{"meta":"js","params":{"expression":"location.href","target_id":"iframe-1"}}
{"meta":"click","params":{"x":640,"y":320,"button":"left","clicks":1}}
{"meta":"type_text","params":{"text":"hello"}}
{"meta":"press_key","params":{"key":"Enter","modifiers":0}}
{"meta":"dispatch_key","params":{"selector":"#search","key":"Enter","event":"keypress"}}
{"meta":"scroll","params":{"x":640,"y":320,"dx":0,"dy":300}}
{"meta":"screenshot","params":{"full":true}}
{"meta":"upload_file","params":{"selector":"#file1","files":["/abs/path/file.txt"],"target_id":"iframe-1"}}
{"meta":"shutdown"}
{"method":"Page.navigate","params":{"url":"https://example.com"},"session_id":"..."}
```

Response shapes:

```json
{"events":[...]}
{"session_id":"..."}
{"session_id":null}
{"dialog":null}
{"ok":true}
{"result":{...}}
{"error":"..."}
```

Unsupported typed meta negotiation:

- If a daemon does not implement a typed helper meta command, it must respond
  with `{"error":"unsupported meta command: <name>"}`.
- `helpers.py` treats that exact prefix as a capability check and falls back to
  raw CDP or client-side behavior where a compatibility path exists.
- Raw CDP requests sent through `cdp(...)` remain part of the compatibility
  contract and are not considered a migration gap.

Supported meta commands:

- `drain_events` -> `{"events":[...]}`
- `session` -> `{"session_id":"..."}` or `{"session_id":null}`
- `set_session` -> `{"session_id":"..."}` or `{"session_id":null}`
- `pending_dialog` -> `{"dialog":null}` or `{"dialog":{...}}`
- `page_info` -> `{"result":{"url":"...","title":"...","w":...}}` or `{"result":{"dialog":{...}}}`
- `current_tab` -> `{"result":{"targetId":"...","title":"...","url":"..."}}`
- `list_tabs` -> `{"result":[{"targetId":"...","title":"...","url":"..."}]}`
- `switch_tab` -> `{"result":"<session-id>"}`
- `new_tab` -> `{"result":"<target-id>"}`
- `ensure_real_tab` -> `{"result":{"targetId":"...","title":"...","url":"..."}}` or `{"result":null}`
- `iframe_target` -> `{"result":"<target-id>"}` or `{"result":null}`
- `wait_for_load` -> `{"result":true}` or `{"result":false}`
- `goto` -> `{"result":{"frameId":"..."}}`
- `js` -> `{"result":<json-value>}`
- `click` -> `{"result":null}`
- `type_text` -> `{"result":null}`
- `press_key` -> `{"result":null}`
- `dispatch_key` -> `{"result":null}`
- `scroll` -> `{"result":null}`
- `screenshot` -> `{"result":"<base64-png>"}`
- `upload_file` -> `{"result":null}`
- `shutdown` -> `{"ok":true}`

All non-meta requests are daemon-forwarded CDP calls and return either
`{"result":{...}}` or `{"error":"..."}`.

## `bhctl` Commands Used By Python

Rust mode in `admin_cli.py` currently relies on these commands and JSON shapes.

### `bhctl daemon-alive [name]`

Output:

```json
{"alive":true,"name":"remote"}
```

### `bhctl ensure-daemon`

Input on stdin:

```json
{"name":"remote","wait":60.0,"env":{"BU_CDP_WS":"wss://...","BU_BROWSER_ID":"..."}}
```

Output:

```json
{"ok":true,"alreadyRunning":false,"name":"remote"}
```

### `bhctl restart-daemon [name]`

Output:

```json
{"ok":true,"name":"remote"}
```

### `bhctl create-browser`

Input: Browser Use `POST /browsers` payload.

Output: Browser Use browser object plus an added `cdpWsUrl` field.

### `bhctl stop-browser <browser-id>`

Output:

```json
{"ok":true,"browserId":"..."}
```

### `bhctl list-cloud-profiles`

Output: array of cloud profile objects with:

- `id`
- `name`
- `userId`
- `cookieDomains`
- `lastUsedAt`

### `bhctl resolve-profile-name <profile-name>`

Output:

```json
{"profileId":"..."}
```

### `bhctl list-local-profiles`

Output: raw `profile-use list --json` array.

### `bhctl sync-local-profile`

Input on stdin:

```json
{
  "profileName":"Default",
  "browser":"Google Chrome",
  "cloudProfileId":"...",
  "includeDomains":["example.com"],
  "excludeDomains":[]
}
```

Output:

```json
{"cloudProfileId":"...","stdout":"...","stderr":"..."}
```

## Compatibility Coverage

The local regression checks for this contract are:

- `cargo test --workspace`
- `python3 -m unittest tests/test_rust_mode_contract.py`
- `python3 scripts/remote_smoke.py` for live Browser Use verification
