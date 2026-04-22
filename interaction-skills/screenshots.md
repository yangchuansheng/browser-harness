# Screenshots

Treat screenshots as an output format, not as the first way to understand page
state. Use them when you need visual proof, a debugging artifact, or a
human-readable snapshot for later review.

## Preferred Order

1. Use `page_info()` and `js()` first when you need structured facts.
2. Use a viewport screenshot when the current visible state is enough.
3. Use a full-page screenshot when the page is taller than the viewport and
   you need the whole scrollable document.
4. Only build targeted section screenshots after you have a stable DOM locator
   or crop region.

## Current Rust Path

The Rust runner and guest boundary now expose screenshot capture directly:

- `bhrun screenshot`
- `bh_guest_sdk::screenshot(full)`

The result is a base64-encoded PNG string. This differs from the Python
compatibility helper, which still writes the decoded PNG bytes to a file path:

```python
from helpers import screenshot

screenshot("/tmp/page.png", full=True)
```

Use the runner/guest path when screenshots are part of a typed workflow or a
Rust/Wasm guest. Use the Python helper when you are doing quick manual shell
inspection.

## Viewport Vs Full Page

Use viewport screenshots when the current visible state is what matters:

- hover state
- focused element
- modal visibility
- above-the-fold verification

Use full-page screenshots when the page is taller than the viewport and the
evidence matters below the fold:

- long feeds
- search result pages
- product pages with lower sections
- after-scroll verification

Do not expect a full-page screenshot to replace structured extraction. It is a
visual artifact, not a robust parser.

## Targeted Section Screenshots

Targeted screenshots are a second step, not a primitive of their own yet.

Recommended pattern:

1. locate the section with `js()`
2. decide whether a crop is really necessary
3. if visual proof is enough, take a full or viewport shot first
4. only then add crop logic in the calling layer if the workflow truly needs it

This keeps the typed host surface small while the stable use cases are still
being discovered.

## Discovery Vs Verification

Use screenshots for discovery when:

- the DOM is confusing
- the page is visually dynamic
- you need to debug what the browser actually rendered

Use screenshots for verification when:

- a flow must leave visible evidence
- a human reviewer needs a visual record
- structured signals alone are not trustworthy enough

Do not use screenshots as the only success signal when a stronger structured
signal exists. Prefer network waits, `page_info()`, or DOM assertions first,
then attach screenshots as supporting proof.

## Acceptance Smoke

The repository smoke for this path is:

- `scripts/bhrun_screenshot_smoke.py`

Primary acceptance is local browser mode:

```bash
BU_BROWSER_MODE=local BU_DAEMON_IMPL=rust python3 scripts/bhrun_screenshot_smoke.py
```

That smoke:

- attaches to the current browser
- makes the page taller with `js()`
- captures both viewport and full-page screenshots through `bhrun screenshot`
- verifies that both outputs are valid PNGs
- verifies that the full-page capture is taller than the viewport capture
