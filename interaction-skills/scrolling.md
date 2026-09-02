# Scrolling

Identify which surface consumes wheel events before you scroll.

The Rust-native path is:

- `bhrun scroll`
- `bh_guest_sdk::scroll(...)`
- `browser-harness scroll`

## Hidden Tabs

Start with the normal `scroll` command. A background tab can leave a wheel
command unanswered. After a proven timeout, use `switch-tab` with
`"activate":true`, retry the same scroll once, and verify the scroll position.
Activation is visible to the user, so keep it as the timeout fallback.

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

```bash
browser-harness scroll <<'JSON'
{"daemon_name":"default","x":300,"y":400,"dy":-320,"dx":0}
JSON
```

## Rules

- choose coordinates over the element that should receive the wheel event
- re-read `page_info()` or DOM state after scrolling
- do not assume page scroll and nested scroll are interchangeable

## Existing Verification

`scroll` is already exercised by the migrated Reddit guest and by the drag/drop
and domain-skill local smokes that depend on stable geometry.
