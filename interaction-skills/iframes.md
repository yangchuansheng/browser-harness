# Iframes

Use same-origin DOM traversal only when the frame actually shares origin with
the top page.

## Same-Origin Case

For same-origin iframes, `js(...)` can often read through
`contentWindow` / `contentDocument` directly.

Example shape:

```python
from scripts._runner_cli import js

title = js(
    "document.querySelector('iframe').contentDocument.querySelector('h1').textContent"
)
print(title)
```

## Coordinate Warning

DOM reads inside the iframe do not change the fact that pointer input still uses
page coordinates. Re-measure element geometry in page space before clicking or
dragging.

## When To Switch Approaches

- same-origin read/write: use `js(...)`
- cross-origin interaction: resolve an iframe target instead
- simple visible click where DOM plumbing is overkill: use page-space input

## Verification

Prefer DOM proof first, then input:

1. confirm the iframe content is loaded
2. read or compute geometry
3. only then click/scroll/drag
