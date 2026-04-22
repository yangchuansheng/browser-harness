# Scrolling

Identify which surface consumes wheel events before you scroll.

The Rust-native path is:

- `bhrun scroll`
- `bh_guest_sdk::scroll(...)`
- `scripts._runner_cli.scroll(...)`

## Split The Cases

Page scroll:

- use when the main document moves
- verify with `page_info()["sy"]`

Nested container scroll:

- use DOM/JS first to find the scroll container
- wheel at coordinates over that container, not arbitrary page coordinates

Virtualized list or dropdown:

- re-measure after opening
- verify loaded items through DOM state, not only wheel events

## Example

```python
from scripts._runner_cli import scroll

scroll(300, 400, dy=-320, dx=0)
```

## Rules

- choose coordinates over the element that should receive the wheel event
- re-read `page_info()` or DOM state after scrolling
- do not assume page scroll and nested scroll are interchangeable

## Existing Verification

`scroll` is already exercised by the migrated Reddit guest and by the drag/drop
and domain-skill local smokes that depend on stable geometry.
