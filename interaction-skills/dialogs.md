# Dialogs

Browser dialogs (`alert`, `confirm`, `prompt`, `beforeunload`) freeze the JS thread. Two approaches depending on timing.

## Detection

`page_info()` auto-surfaces any open dialog: if one is pending it returns `{"dialog": {"type", "message", ...}}` instead of the usual viewport dict (because the page's JS is frozen anyway). So if you call `page_info()` after an action and see a `dialog` key, handle it before doing anything else.

When you expect a dialog and need a blocking wait instead of post-hoc polling,
prefer the Rust runner path:

- `bhrun wait-for-dialog`
- `bh_guest_sdk::wait_for_dialog(...)` in Rust/Wasm guests
- `bhrun handle-dialog`
- `bh_guest_sdk::handle_dialog(...)` in Rust/Wasm guests

That gives you a scoped runner-owned wait on `Page.javascriptDialogOpening`
without depending on the destructive `drain_events()` buffer, plus a typed
follow-up path to accept or dismiss the dialog.

## Reactive: accept or dismiss (preferred)

Works even when JS is frozen. Handles all dialog types including `beforeunload`.

```bash
# Accept / click OK
bhrun handle-dialog <<'JSON'
{"daemon_name":"default","action":"accept"}
JSON

# Cancel / click Cancel
bhrun handle-dialog <<'JSON'
{"daemon_name":"default","action":"dismiss"}
JSON

# Prompt dialogs can also submit text
bhrun handle-dialog <<'JSON'
{"daemon_name":"default","action":"accept","prompt_text":"typed value"}
JSON
```

```rust
use bh_guest_sdk::{handle_dialog, wait_for_dialog};

let opened = wait_for_dialog(Some("prompt"), None, None, 5_000, 100)?;
if opened.matched {
    handle_dialog("accept", Some("typed value"))?;
}
```

Undetectable by antibot — no JS injected into the page.

## Proactive: stub via JS

Prevents dialogs from ever appearing. Good when you expect multiple `alert()`/`confirm()` calls in sequence.

```python
js("""
window.__dialogs__=[];
window.alert=m=>window.__dialogs__.push(String(m));
window.confirm=m=>{window.__dialogs__.push(String(m));return true;};
window.prompt=(m,d)=>{window.__dialogs__.push(String(m));return d||'';};
""")
# ... do actions that trigger dialogs ...
msgs = js("window.__dialogs__||[]")
```

Tradeoffs:
- Stubs are lost on page navigation -- must re-run the snippet
- `confirm()` always returns `true` (auto-approves)
- Detectable by antibot (`window.alert.toString()` reveals non-native code)
- Does NOT handle `beforeunload`

## beforeunload specifically

Fires when navigating away from a page with unsaved changes (forms, editors, upload pages). The page freezes until the user clicks Leave/Stay.

```bash
bhrun goto <<'JSON'
{"daemon_name":"default","url":"https://new-url.com"}
JSON

bhrun handle-dialog <<'JSON'
{"daemon_name":"default","action":"accept"}
JSON
```

Legacy fallback:

- archived `helpers.py` / raw `Page.handleJavaScriptDialog` calls are
  historical-only, not the primary path.
