# Cookies

Separate browser cookie state from page-visible DOM state.

The Rust-native path is:

- `bhrun get-cookies`
- `bhrun set-cookies`
- `browser-harness get-cookies`
- `browser-harness set-cookies`
- `bh_guest_sdk::get_cookies(...)`
- `bh_guest_sdk::set_cookies(...)`

## Rules

1. Use `set-cookies` to write browser cookies before a navigation or auth flow.
2. Use `get-cookies` to read what the browser actually stored.
3. Use `document.cookie` only as a page-level cross-check, not as the source of
   truth for `HttpOnly` or cross-site cookies.

## Example

```python
from scripts._runner_cli import get_cookies, set_cookies

set_cookies(
    [{
        "name": "session",
        "value": "token",
        "url": "https://example.com",
        "secure": True,
        "sameSite": "Lax",
    }]
)

print(get_cookies(["https://example.com"]))
```

```bash
bhrun set-cookies <<'JSON'
{"daemon_name":"default","cookies":[{"name":"session","value":"token","url":"https://example.com","secure":true}]}
JSON
```

## What To Verify

After `set-cookies`, verify one or more of:

- `get-cookies` returns the expected name/value/domain/path
- the next navigation behaves as authenticated
- `document.cookie` shows the expected non-`HttpOnly` cookie

## Local Acceptance

- `rust/bins/bhsmoke` with the `cookies` scenario

```bash
BU_BROWSER_MODE=local BU_DAEMON_IMPL=rust cargo run --quiet --manifest-path rust/Cargo.toml --bin bhsmoke -- cookies
```

That smoke:

- writes a unique cookie for `example.com`
- confirms page JS can see the cookie when appropriate
- confirms `get-cookies` returns the same browser-stored value
