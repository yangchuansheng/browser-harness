# Drag And Drop

Split pointer-driven dragging from file-drop and DOM-specific drag systems.

The Rust-native low-level pointer path is:

- `bhrun mouse-move`
- `bhrun mouse-down`
- `bhrun mouse-up`
- `browser-harness mouse-move|mouse-down|mouse-up`
- `bh_guest_sdk::mouse_move(...)`
- `bh_guest_sdk::mouse_down(...)`
- `bh_guest_sdk::mouse_up(...)`

## Use These Primitives When

Use low-level pointer events for:

- sliders
- drag handles
- canvas tools
- sortable UIs that react to actual mouse movement

Typical sequence:

1. move to the element
2. press the mouse button
3. move while `buttons=1`
4. release the mouse button

## Example

```bash
browser-harness mouse-move <<'JSON'
{"daemon_name":"default","x":100,"y":200,"buttons":0}
JSON
browser-harness mouse-down <<'JSON'
{"daemon_name":"default","x":100,"y":200,"button":"left","buttons":1,"click_count":1}
JSON
browser-harness mouse-move <<'JSON'
{"daemon_name":"default","x":320,"y":200,"buttons":1}
JSON
browser-harness mouse-up <<'JSON'
{"daemon_name":"default","x":320,"y":200,"button":"left","buttons":0,"click_count":1}
JSON
```

## When These Primitives Are Not Enough

Do not expect low-level pointer events alone to solve:

- HTML5 drag-and-drop flows that depend on `DataTransfer`
- file-drop zones that really want uploaded files
- deeply custom component trees where DOM injection is simpler than pointer
  choreography

In those cases:

- prefer `upload-file` for file inputs
- use DOM/JS helpers when the site expects drag data, not only movement

## Local Acceptance

- `rust/bins/bhsmoke` with the `drag` scenario

```bash
BU_BROWSER_MODE=local BU_DAEMON_IMPL=rust cargo run --quiet --manifest-path rust/Cargo.toml --bin bhsmoke -- drag
```

That smoke:

- injects a pointer-driven drag handle into `about:blank`
- drags it with `mouse-move` / `mouse-down` / `mouse-up`
- verifies the page recorded `down -> move -> move -> up`
- verifies the handle actually moved across the track
