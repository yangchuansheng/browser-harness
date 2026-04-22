# Print As PDF

Split CDP-native PDF export from visible browser print UI.

The Rust-native path is:

- `bhrun print-pdf`
- `browser-harness print-pdf`
- `bh_guest_sdk::print_pdf(landscape)`

## Preferred Path

Use `print-pdf` when you want the current page rendered directly through
`Page.printToPDF`.

That is the right tool when:

- you only need a PDF artifact
- the site is already in the right print-ready state
- you do not need to automate the browser's visible print dialog

## Example

```bash
browser-harness print-pdf <<'JSON' | jq -r . | base64 --decode > /tmp/page.pdf
{"daemon_name":"default","landscape":false}
JSON
```

## When Not To Use It

Do not confuse `print-pdf` with clicking a visible "Print" button.

If a site only reveals printable content after a print-specific UI flow, you
may still need:

1. browser actions to open that view
2. then `print-pdf`

Do not automate the native OS print dialog through this primitive. This helper
stops at CDP PDF generation.

## Verification

Verify the result by:

- base64-decoding the response
- checking the `%PDF-` header
- comparing byte size across portrait vs landscape if layout should differ

## Local Acceptance

- `rust/bins/bhsmoke` with the `print-pdf` scenario

```bash
BU_BROWSER_MODE=local BU_DAEMON_IMPL=rust cargo run --quiet --manifest-path rust/Cargo.toml --bin bhsmoke -- print-pdf
```

That smoke:

- navigates to `example.com`
- renders both portrait and landscape PDFs
- verifies both outputs are valid PDFs and not identical
