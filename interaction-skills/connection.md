# Connection & Tab Visibility

Treat connection recovery and visible-tab recovery as runner concerns first.

The Rust-native path is:

- `browser-harness ensure-daemon`
- `bhrun list-tabs`
- `bhrun current-tab`
- `bhrun ensure-real-tab`
- `bhrun switch-tab`
- `bhrun close-tab`

## The Real Problem

Fresh Chrome can expose internal page targets such as:

- `chrome://inspect`
- `chrome://omnibox-popup.top-chrome/`

If the daemon attaches there, later navigation may succeed in CDP while the user
still sees the wrong surface.

`ensure-real-tab` is the recovery primitive for that case. Named local/CDP
daemons keep a dedicated background target so parallel daemon names retain
independent automation state.

## Preferred Startup Sequence

1. start or confirm the daemon
2. list tabs
3. call `ensure-real-tab`
4. navigate or switch in the attached background tab

```bash
browser-harness ensure-daemon

bhrun list-tabs <<'JSON'
{"daemon_name":"default","include_internal":false}
JSON

bhrun ensure-real-tab <<'JSON'
{"daemon_name":"default"}
JSON

bhrun switch-tab <<'JSON'
{"daemon_name":"default","target_id":"<target-id>","activate":true}
JSON
```

## Rules

- prefer `ensure-real-tab` before a browser-first workflow starts
- use `switch-tab` to attach to a known target while preserving Chrome's visible tab
- set `activate=true` for an explicit visible switch or verified
  visibility-dependent rendering
- treat `new-tab` as a background creation-and-attach primitive
- call `close-tab` for temporary tabs once the workflow no longer needs them
- if `page_info()` shows `w=0` or `h=0`, recover the attachment instead of
  continuing blindly

Named daemon recovery reuses the selected target while it exists, falls back to
the daemon's dedicated target, and creates one shared background replacement
after the target disappears. Daemon shutdown leaves working tabs open for reuse.

## Verification

Use `page_info()` or `current-tab` after recovery and confirm:

- the URL is a real page
- the viewport dimensions are non-zero
- the tab is the one you intended to automate
