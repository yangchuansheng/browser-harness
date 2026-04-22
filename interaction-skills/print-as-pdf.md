# Print As PDF

Split CDP-native PDF export from visible browser print UI.

The Rust-native path is:

- `bhrun print-pdf`
- `browser-harness print-pdf`
- `bh_guest_sdk::print_pdf(landscape)`
- `scripts._runner_cli.print_pdf(...)`

## Preferred Path

Use `print-pdf` when you want the current page rendered directly through
`Page.printToPDF`.

That is the right tool when:

- you only need a PDF artifact
- the site is already in the right print-ready state
- you do not need to automate the browser's visible print dialog

## Example

```python
from scripts._runner_cli import print_pdf

print_pdf("/tmp/page.pdf", landscape=False)
```

```bash
bhrun print-pdf <<'JSON'
{"daemon_name":"default","landscape":true}
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

- `scripts/bhrun_print_pdf_smoke.py`

```bash
BU_BROWSER_MODE=local BU_DAEMON_IMPL=rust python3 scripts/bhrun_print_pdf_smoke.py
```

That smoke:

- navigates to `example.com`
- renders both portrait and landscape PDFs
- verifies both outputs are valid PDFs and not identical
