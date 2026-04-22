# Rust Workspace

This workspace is the starting point for the Browser Harness rewrite.

The near-term goal is a Rust daemon/core that preserves the current Python
workflow. The long-term goal is a Rust host with a WASM guest layer.

Current status:

- Phase 1 hybrid rewrite is complete: Rust owns the daemon/runtime core and Python remains the compatibility shell
- Rust daemon connects to local or remote CDP and serves the existing Unix socket contract
- first typed helper operations are implemented in the Rust daemon: page info, tab listing/current tab, tab switching, new-tab creation, real-tab recovery, iframe lookup, load waiting, JS evaluation, goto, screenshot capture, viewport control, PDF export, cookie read/write, download configuration, low-level input primitives, drag primitives, DOM key dispatch, and file upload
- remote-browser shutdown parity is implemented in the Rust daemon
- local regression tests cover protocol, discovery, remote stop requests, daemon buffer behavior, and Python Rust-mode compatibility paths
- site-dependent domain-skill acceptance now has passing local browser smokes via `DevToolsActivePort`, including the GitHub trending, Reddit post, Product Hunt homepage, Letterboxd popular, Spotify search, and Etsy search guest slices
- Browser Use remote remains useful for simple runner/plumbing smokes, but site-dependent guest verification against origins such as GitHub, Reddit, Product Hunt, and Letterboxd is currently best-effort because cloud navigation to those sites has been intermittently unreliable
- the first preview guest-execution slice exists via `bh-wasm-host`, `bhrun`, and [docs/wasm-runner-design.md](../docs/wasm-runner-design.md)
- the new Rust-native top-level CLI facade is `browser-harness`, which forwards admin commands to `bhctl` and runner/helper commands to `bhrun`
- the repo-owned runner/admin verification smokes for the common helper surface now live in `bins/bhsmoke`
- `bhrun` now has a first persistent guest-runner preview via `serve-guest`, plus the runner-local `wait` utility for browser-free guest verification
- the first Rust guest authoring path now exists via `bh-guest-sdk` and `guests/rust-navigate-and-read`
- the persistent browser-state sample guest is now also available as a compiled Rust Wasm guest via `guests/rust-persistent-browser-state`
- `bh-guest-sdk` now also covers typed tab/session control and the event-wait family, with compiled workflow samples in `guests/rust-tab-response-workflow` and `guests/rust-event-waits-sdk`
- the guest SDK and runner now also expose `wait_for_load`, `ensure_real_tab`, `iframe_target`, `click`, `mouse_move`, `mouse_down`, `mouse_up`, `type_text`, `press_key`, `dispatch_key`, `scroll`, `set_viewport`, `print_pdf`, `screenshot`, `handle_dialog`, `upload_file`, `get_cookies`, `set_cookies`, `configure_downloads`, `wait_for_download`, runner-owned `http_get`, request-side `wait_for_request`, and the capability-gated raw CDP escape hatch `cdp_raw`
- the first skill-shaped Rust/Wasm guest now exists via `guests/rust-github-trending`, which ports the browser-trending slice of `domain-skills/github/scraping.md`
- a second skill-shaped Rust/Wasm guest now exists via `guests/rust-reddit-post-scrape`, which ports the browser DOM extraction slice of `domain-skills/reddit/scraping.md`
- a third skill-shaped Rust/Wasm guest now exists via `guests/rust-producthunt-homepage`, which ports the homepage feed slice of `domain-skills/producthunt/scraping.md` with a `new_tab()`-first flow and a fallback extractor for the current homepage DOM
- a fourth skill-shaped Rust/Wasm guest now exists via `guests/rust-letterboxd-popular`, which ports the browser-only popular browse slice of `domain-skills/letterboxd/scraping.md`
- a fifth and sixth browser-first skill-shaped guests now exist via `guests/rust-spotify-search` and `guests/rust-etsy-search`
- the first HTTP-owned skill-shaped guests now exist via `guests/rust-metacritic-game-scores`, `guests/rust-walmart-search`, and `guests/rust-tradingview-symbol-search`

Compatibility contract:

- [docs/rust-compat-contract.md](../docs/rust-compat-contract.md)

Quick verification:

```bash
cd rust
cargo test --workspace
cargo run --quiet --bin browser-harness -- --help
```

WASM design scaffold:

```bash
cd rust
cargo run --quiet --bin bhrun -- manifest
cargo run --quiet --bin bhrun -- sample-config
cargo run --quiet --bin bhrun -- run-guest guests/navigate_and_read.wat <<'JSON'
{"daemon_name":"default","guest_module":"guests/navigate_and_read.wat","granted_operations":["goto","wait_for_load_event","page_info","js"],"allow_http":false,"allow_raw_cdp":false,"persistent_guest_state":true}
JSON
rustup target add --toolchain stable-x86_64-unknown-linux-gnu wasm32-unknown-unknown
cargo +stable build --release --target wasm32-unknown-unknown --manifest-path guests/rust-navigate-and-read/Cargo.toml
cargo run --quiet --bin bhrun -- run-guest guests/rust-navigate-and-read/target/wasm32-unknown-unknown/release/rust_navigate_and_read_guest.wasm <<'JSON'
{"daemon_name":"default","guest_module":"guests/rust-navigate-and-read/target/wasm32-unknown-unknown/release/rust_navigate_and_read_guest.wasm","granted_operations":["goto","wait_for_load_event","page_info","js"],"allow_http":false,"allow_raw_cdp":false,"persistent_guest_state":true}
JSON
cargo run --quiet --bin bhrun -- wait <<'JSON'
{"duration_ms":1}
JSON
cargo run --quiet --bin bhrun -- http-get <<'JSON'
{"url":"https://open.spotify.com/oembed?url=https://open.spotify.com/track/4PTG3Z6ehGkBFwjybzWkR8","timeout":20.0}
JSON
cat <<'NDJSON' | cargo run --quiet --bin bhrun -- serve-guest guests/persistent_counter.wat
{"command":"start","config":{"daemon_name":"default","guest_module":"guests/persistent_counter.wat","granted_operations":["wait"],"allow_http":false,"allow_raw_cdp":false,"persistent_guest_state":true}}
{"command":"run"}
{"command":"run"}
{"command":"stop"}
NDJSON
cargo +stable build --release --target wasm32-unknown-unknown --manifest-path guests/rust-persistent-browser-state/Cargo.toml
cat <<'NDJSON' | cargo run --quiet --bin bhrun -- serve-guest guests/rust-persistent-browser-state/target/wasm32-unknown-unknown/release/rust_persistent_browser_state_guest.wasm
{"command":"start","config":{"daemon_name":"default","guest_module":"guests/rust-persistent-browser-state/target/wasm32-unknown-unknown/release/rust_persistent_browser_state_guest.wasm","granted_operations":["goto","wait_for_load_event","js","page_info"],"allow_http":false,"allow_raw_cdp":false,"persistent_guest_state":true}}
{"command":"run"}
{"command":"run"}
{"command":"stop"}
NDJSON
cargo +stable build --release --target wasm32-unknown-unknown --manifest-path guests/rust-tab-response-workflow/Cargo.toml
cargo run --quiet --bin bhrun -- run-guest guests/rust-tab-response-workflow/target/wasm32-unknown-unknown/release/rust_tab_response_workflow_guest.wasm <<'JSON'
{"daemon_name":"default","guest_module":"guests/rust-tab-response-workflow/target/wasm32-unknown-unknown/release/rust_tab_response_workflow_guest.wasm","granted_operations":["current_tab","list_tabs","new_tab","switch_tab","current_session","goto","wait_for_response","page_info","js"],"allow_http":false,"allow_raw_cdp":false,"persistent_guest_state":true}
JSON
cargo +stable build --release --target wasm32-unknown-unknown --manifest-path guests/rust-github-trending/Cargo.toml
cargo run --quiet --bin bhrun -- run-guest guests/rust-github-trending/target/wasm32-unknown-unknown/release/rust_github_trending_guest.wasm <<'JSON'
{"daemon_name":"default","guest_module":"guests/rust-github-trending/target/wasm32-unknown-unknown/release/rust_github_trending_guest.wasm","granted_operations":["ensure_real_tab","goto","wait_for_load","wait","page_info","js"],"allow_http":false,"allow_raw_cdp":false,"persistent_guest_state":true}
JSON
cargo +stable build --release --target wasm32-unknown-unknown --manifest-path guests/rust-reddit-post-scrape/Cargo.toml
cargo run --quiet --bin bhrun -- run-guest guests/rust-reddit-post-scrape/target/wasm32-unknown-unknown/release/rust_reddit_post_scrape_guest.wasm <<'JSON'
{"daemon_name":"default","guest_module":"guests/rust-reddit-post-scrape/target/wasm32-unknown-unknown/release/rust_reddit_post_scrape_guest.wasm","granted_operations":["ensure_real_tab","goto","wait_for_load","wait","scroll","page_info","js"],"allow_http":false,"allow_raw_cdp":false,"persistent_guest_state":true}
JSON
cargo run --quiet --bin bhrun -- current-tab <<'JSON'
{"daemon_name":"default"}
JSON
cargo run --quiet --bin bhrun -- list-tabs <<'JSON'
{"daemon_name":"default","include_internal":true}
JSON
cargo run --quiet --bin bhrun -- new-tab <<'JSON'
{"daemon_name":"default","url":"https://example.com"}
JSON
cargo run --quiet --bin bhrun -- switch-tab <<'JSON'
{"daemon_name":"default","target_id":"<target-id>"}
JSON
cargo run --quiet --bin bhrun -- ensure-real-tab <<'JSON'
{"daemon_name":"default"}
JSON
cargo run --quiet --bin bhrun -- iframe-target <<'JSON'
{"daemon_name":"default","url_substr":"github.com"}
JSON
cargo run --quiet --bin bhrun -- page-info <<'JSON'
{"daemon_name":"default"}
JSON
cargo run --quiet --bin bhrun -- goto <<'JSON'
{"daemon_name":"default","url":"https://example.com"}
JSON
cargo run --quiet --bin bhrun -- wait-for-load <<'JSON'
{"daemon_name":"default","timeout":15.0}
JSON
cargo run --quiet --bin bhrun -- js <<'JSON'
{"daemon_name":"default","expression":"location.href"}
JSON
cargo run --quiet --bin bhrun -- click <<'JSON'
{"daemon_name":"default","x":100,"y":200,"button":"left","clicks":1}
JSON
cargo run --quiet --bin bhrun -- type-text <<'JSON'
{"daemon_name":"default","text":"hello"}
JSON
cargo run --quiet --bin bhrun -- press-key <<'JSON'
{"daemon_name":"default","key":"Enter","modifiers":0}
JSON
cargo run --quiet --bin bhrun -- dispatch-key <<'JSON'
{"daemon_name":"default","selector":"#search","key":"Tab","event":"keydown"}
JSON
cargo run --quiet --bin bhrun -- scroll <<'JSON'
{"daemon_name":"default","x":100,"y":200,"dy":-300,"dx":0}
JSON
cargo run --quiet --bin bhrun -- set-viewport <<'JSON'
{"daemon_name":"default","width":1280,"height":800,"device_scale_factor":1.0,"mobile":false}
JSON
cargo run --quiet --bin bhrun -- upload-file <<'JSON'
{"daemon_name":"default","selector":"#file1","files":["/abs/path/file.txt"]}
JSON
cargo run --quiet --bin bhrun -- current-session <<'JSON'
{"daemon_name":"default"}
JSON
cargo run --quiet --bin bhrun -- wait-for-event <<'JSON'
{"daemon_name":"default","filter":{"method":"Page.loadEventFired"}}
JSON
cargo run --quiet --bin bhrun -- watch-events <<'JSON'
{"daemon_name":"default","filter":{"session_id":"<current-session-id>"},"timeout_ms":2000,"max_events":10}
JSON
cargo run --quiet --bin bhrun -- wait-for-load-event <<'JSON'
{"daemon_name":"default","session_id":"<current-session-id>"}
JSON
cargo run --quiet --bin bhrun -- wait-for-request <<'JSON'
{"daemon_name":"default","session_id":"<current-session-id>","url":"https://example.com/api","method":"POST"}
JSON
cargo run --quiet --bin bhrun -- wait-for-response <<'JSON'
{"daemon_name":"default","session_id":"<current-session-id>","url":"https://example.com/api","status":200}
JSON
cargo run --quiet --bin bhrun -- wait-for-console <<'JSON'
{"daemon_name":"default","session_id":"<current-session-id>","type":"log","text":"ready"}
JSON
cargo run --quiet --bin bhrun -- wait-for-dialog <<'JSON'
{"daemon_name":"default","session_id":"<current-session-id>","type":"alert","message":"ready"}
JSON
```

Python compatibility tests:

```bash
python3 -m unittest tests/test_rust_mode_contract.py
```

Live remote smoke test:

```bash
BROWSER_USE_API_KEY=... cargo run --quiet --manifest-path rust/Cargo.toml --bin bhsmoke -- remote
```

Live `bhrun wait-for-event` smoke:

```bash
BROWSER_USE_API_KEY=... cargo run --quiet --manifest-path rust/Cargo.toml --bin bhsmoke -- wait-for-load-event
```

Live `bhrun watch-events` smoke:

```bash
BROWSER_USE_API_KEY=... cargo run --quiet --manifest-path rust/Cargo.toml --bin bhsmoke -- watch-events
```

Live `bhrun wait-for-response` smoke:

```bash
BROWSER_USE_API_KEY=... cargo run --quiet --manifest-path rust/Cargo.toml --bin bhsmoke -- wait-for-response
```

Local `bhrun wait-for-request` smoke:

```bash
BU_BROWSER_MODE=local BU_DAEMON_IMPL=rust cargo run --quiet --manifest-path rust/Cargo.toml --bin bhsmoke -- wait-for-request
```

Local `bhrun set-viewport` smoke:

```bash
BU_BROWSER_MODE=local BU_DAEMON_IMPL=rust cargo run --quiet --manifest-path rust/Cargo.toml --bin bhsmoke -- set-viewport
```

Live `bhrun wait-for-console` smoke:

```bash
BROWSER_USE_API_KEY=... cargo run --quiet --manifest-path rust/Cargo.toml --bin bhsmoke -- wait-for-console
```

Live `bhrun wait-for-dialog` smoke:

```bash
BROWSER_USE_API_KEY=... cargo run --quiet --manifest-path rust/Cargo.toml --bin bhsmoke -- wait-for-dialog
```

Live `bhrun screenshot` smoke:

```bash
BROWSER_USE_API_KEY=... cargo run --quiet --manifest-path rust/Cargo.toml --bin bhsmoke -- screenshot
```

Local `bhrun` runner-helper smokes:

```bash
BU_BROWSER_MODE=local BU_DAEMON_IMPL=rust cargo run --quiet --manifest-path rust/Cargo.toml --bin bhsmoke -- screenshot
BU_BROWSER_MODE=local BU_DAEMON_IMPL=rust cargo run --quiet --manifest-path rust/Cargo.toml --bin bhsmoke -- print-pdf
BU_BROWSER_MODE=local BU_DAEMON_IMPL=rust cargo run --quiet --manifest-path rust/Cargo.toml --bin bhsmoke -- cookies
BU_BROWSER_MODE=local BU_DAEMON_IMPL=rust cargo run --quiet --manifest-path rust/Cargo.toml --bin bhsmoke -- wait-for-download
BU_BROWSER_MODE=local BU_DAEMON_IMPL=rust cargo run --quiet --manifest-path rust/Cargo.toml --bin bhsmoke -- drag
BU_BROWSER_MODE=local BU_DAEMON_IMPL=rust cargo run --quiet --manifest-path rust/Cargo.toml --bin bhsmoke -- upload-file
```

Local `bhrun` action smoke:

```bash
BU_BROWSER_MODE=local BU_DAEMON_IMPL=rust cargo run --quiet --manifest-path rust/Cargo.toml --bin bhsmoke -- actions
```

Local `bhrun` tab/session smoke:

```bash
BU_BROWSER_MODE=local BU_DAEMON_IMPL=rust cargo run --quiet --manifest-path rust/Cargo.toml --bin bhsmoke -- tabs
```

Live `bhrun` guest smokes:

```bash
BROWSER_USE_API_KEY=... cargo run --quiet --manifest-path rust/Cargo.toml --bin bhsmoke -- guest-run
BROWSER_USE_API_KEY=... BU_GUEST_PATH="$PWD/rust/guests/rust-navigate-and-read/target/wasm32-unknown-unknown/release/rust_navigate_and_read_guest.wasm" cargo run --quiet --manifest-path rust/Cargo.toml --bin bhsmoke -- guest-run
BROWSER_USE_API_KEY=... BU_GUEST_PATH="$PWD/rust/guests/rust-navigate-and-read/target/wasm32-unknown-unknown/release/rust_navigate_and_read_guest.wasm" cargo run --quiet --manifest-path rust/Cargo.toml --bin bhsmoke -- guest-serve
BROWSER_USE_API_KEY=... cargo run --quiet --manifest-path rust/Cargo.toml --bin bhsmoke -- tab-response-guest
BU_BROWSER_MODE=local BU_DAEMON_IMPL=rust cargo run --quiet --manifest-path rust/Cargo.toml --bin bhsmoke -- event-waits-guest
BU_BROWSER_MODE=local BU_DAEMON_IMPL=rust cargo run --quiet --manifest-path rust/Cargo.toml --bin bhsmoke -- raw-cdp-guest
BROWSER_USE_API_KEY=... cargo run --quiet --manifest-path rust/Cargo.toml --bin bhsmoke -- github-trending-guest
BROWSER_USE_API_KEY=... cargo run --quiet --manifest-path rust/Cargo.toml --bin bhsmoke -- reddit-guest
```

Local site-dependent domain-skill guest smokes:

```bash
BU_BROWSER_MODE=local BU_DAEMON_IMPL=rust cargo run --quiet --manifest-path rust/Cargo.toml --bin bhsmoke -- github-trending-guest
BU_BROWSER_MODE=local BU_DAEMON_IMPL=rust cargo run --quiet --manifest-path rust/Cargo.toml --bin bhsmoke -- reddit-guest
BU_BROWSER_MODE=local BU_DAEMON_IMPL=rust cargo run --quiet --manifest-path rust/Cargo.toml --bin bhsmoke -- producthunt-guest
BU_BROWSER_MODE=local BU_DAEMON_IMPL=rust cargo run --quiet --manifest-path rust/Cargo.toml --bin bhsmoke -- letterboxd-popular-guest
BU_BROWSER_MODE=local BU_DAEMON_IMPL=rust cargo run --quiet --manifest-path rust/Cargo.toml --bin bhsmoke -- spotify-search-guest
BU_BROWSER_MODE=local BU_DAEMON_IMPL=rust cargo run --quiet --manifest-path rust/Cargo.toml --bin bhsmoke -- etsy-search-guest
```

Runner-local `http_get` domain-skill guest smokes:

```bash
python3 scripts/bhrun_metacritic_game_scores_guest_smoke.py
python3 scripts/bhrun_walmart_search_guest_smoke.py
python3 scripts/bhrun_tradingview_symbol_search_guest_smoke.py
```

Live `bhrun serve-guest` smoke:

```bash
BROWSER_USE_API_KEY=... cargo run --quiet --manifest-path rust/Cargo.toml --bin bhsmoke -- persistent-guest-browser
BROWSER_USE_API_KEY=... BU_GUEST_PATH="$PWD/rust/guests/rust-persistent-browser-state/target/wasm32-unknown-unknown/release/rust_persistent_browser_state_guest.wasm" cargo run --quiet --manifest-path rust/Cargo.toml --bin bhsmoke -- persistent-guest-browser
```

Local `bhrun serve-guest` smoke:

```bash
cargo run --quiet --manifest-path rust/Cargo.toml --bin bhsmoke -- persistent-guest
```

Remote GitHub domain-skill acceptance smoke (best-effort):

```bash
BROWSER_USE_API_KEY=... python3 scripts/domain_skill_github_smoke.py
```
