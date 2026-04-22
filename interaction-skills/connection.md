# Connection & Tab Visibility

Treat connection recovery and visible-tab recovery as runner concerns first.

The Rust-native path is:

- `browser-harness ensure-daemon`
- `bhrun list-tabs`
- `bhrun current-tab`
- `bhrun ensure-real-tab`
- `bhrun switch-tab`

## The Real Problem

Fresh Chrome can expose internal page targets such as:

- `chrome://inspect`
- `chrome://omnibox-popup.top-chrome/`

If the daemon attaches there, later navigation may succeed in CDP while the user
still sees the wrong surface.

`ensure-real-tab` is the recovery primitive for that case.

## Preferred Startup Sequence

1. start or confirm the daemon
2. list tabs
3. call `ensure-real-tab`
4. only then navigate or switch

```bash
browser-harness ensure-daemon

bhrun list-tabs <<'JSON'
{"daemon_name":"default","include_internal":false}
JSON

bhrun ensure-real-tab <<'JSON'
{"daemon_name":"default"}
JSON
```

## Rules

- prefer `ensure-real-tab` before a browser-first workflow starts
- use `switch-tab` when you already know the target id you want
- treat `new-tab` as a creation primitive, not as visibility proof
- if `page_info()` shows `w=0` or `h=0`, recover the attachment instead of
  continuing blindly

## Verification

Use `page_info()` or `current-tab` after recovery and confirm:

- the URL is a real page
- the viewport dimensions are non-zero
- the tab is the one you intended to automate
