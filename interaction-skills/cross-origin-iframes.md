# Cross-Origin Iframes

Use target resolution first, then operate inside the iframe deliberately.

The Rust-native path is:

- `bhrun iframe-target`
- `bh_guest_sdk::iframe_target(...)`
- `bhrun upload-file` with `target_id`
- `bhrun js` with `target_id` where applicable

## Preferred Flow

1. resolve the iframe target by URL substring
2. use the returned `target_id` for iframe-scoped work
3. fall back to coordinate input only when iframe DOM work is harder than the
   action itself

```bash
bhrun iframe-target <<'JSON'
{"daemon_name":"default","url_substr":"accounts.google.com"}
JSON
```

## Rules

- prefer `iframe-target` over guessing iframe coordinates
- treat the iframe as a separate target, not just a DOM node
- keep page coordinates and iframe-local DOM work conceptually separate

## Verification

After resolving the iframe target:

- run a scoped `js` call if you need DOM proof
- or use typed helpers such as `upload-file` against that target

The cross-origin iframe path is already part of the Rust runner surface even
though it does not need a dedicated smoke of its own yet.
