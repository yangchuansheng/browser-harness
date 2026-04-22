# Tabs

Use typed tab control first. Do not drop to raw CDP unless the typed surface is
missing something specific.

The Rust-native path is:

- `bhrun list-tabs`
- `bhrun current-tab`
- `bhrun new-tab`
- `bhrun switch-tab`
- `bh_guest_sdk::{list_tabs,current_tab,new_tab,switch_tab}`

## Recommended Flow

```bash
bhrun new-tab <<'JSON'
{"daemon_name":"default","url":"https://example.com"}
JSON

bhrun switch-tab <<'JSON'
{"daemon_name":"default","target_id":"<target-id>"}
JSON
```

## What These Helpers Mean

- `list-tabs`: enumerate page targets
- `current-tab`: inspect the currently attached target
- `new-tab`: create a new page target
- `switch-tab`: attach to and activate a known target

## Rules

- use `list-tabs(include_internal=false)` for user-facing work
- use `switch-tab` when you already know the target id
- use `ensure-real-tab` if you suspect the daemon is attached to an internal tab
- re-check `current-tab` or `page_info()` after switching if the flow is
  layout-sensitive

## Local Verification

The tab/session helpers are already exercised by:

- `rust/bins/bhsmoke` with the `tab-response-guest` scenario
- `scripts/bhrun_github_trending_guest_smoke.py`
- `scripts/bhrun_letterboxd_popular_guest_smoke.py`
