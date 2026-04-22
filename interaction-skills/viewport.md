# Viewport

Treat viewport size as part of the test fixture, not as incidental browser
state. If a workflow depends on layout, scroll geometry, screenshots, or raw
coordinates, set the viewport first and keep it stable for the whole slice.

## Preferred Order

1. Decide whether the flow is desktop-first or mobile-first before any
   coordinate click or screenshot.
2. Set the viewport before `page_info()`, screenshot capture, or layout
   assertions.
3. Re-read `page_info()` after changing viewport because width, height, and
   page scroll size all change with the new layout.
4. Only use raw `click(x, y, ...)` after the viewport and scroll position are
   both fixed.

## Current Rust Path

The Rust-native path now exposes typed viewport control directly:

- `bhrun set-viewport`
- `browser-harness set-viewport`
- `bh_guest_sdk::set_viewport(...)`

Repo-local Python scripts should use the Rust-backed shim in
`scripts/_runner_cli.py`:

```python
from scripts._runner_cli import page_info, set_viewport

set_viewport(1280, 800, device_scale_factor=1.0, mobile=False)
print(page_info())   # {'w': 1280, 'h': 800, ...}
```

This is now the preferred path. The old Python helper shell is compatibility
only.

## Desktop Vs Mobile

Use a desktop viewport when:

- selectors are documented against the normal desktop layout
- the site collapses content aggressively on narrow widths
- you need stable screenshot dimensions

Use a mobile viewport when:

- the real workflow is explicitly mobile-only
- the site hides or replaces controls behind a mobile breakpoint
- you are validating responsive layout behavior on purpose

Do not assume a narrow width alone is enough proof. Always verify the active
layout with `page_info()` or `js()` after setting the viewport.

## Geometry Rules

Viewport changes affect more than visible size:

- CSS breakpoints can move or hide elements
- scroll height and scroll width can change
- raw pixel coordinates stop being portable across runs

So the safe sequence is:

1. set viewport
2. confirm `page_info()["w"]` / `page_info()["h"]`
3. scroll if needed
4. only then use coordinate clicks or capture screenshots

If you change viewport mid-flow, treat all earlier coordinates as invalid and
recompute them.

## Acceptance Smoke

The repository smoke for this path is:

- `rust/bins/bhsmoke` with the `set-viewport` scenario

Primary acceptance is local browser mode:

```bash
BU_BROWSER_MODE=local BU_DAEMON_IMPL=rust cargo run --quiet --manifest-path rust/Cargo.toml --bin bhsmoke -- set-viewport
```

That smoke:

- attaches to the current browser
- sets a desktop viewport and verifies `page_info()` matches it
- sets a mobile-width viewport with a different device scale factor
- verifies the page dimensions and `window.devicePixelRatio`
- restores the original viewport before shutdown
