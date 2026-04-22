# Rust Migration Backlog

This backlog tracks the Rust/Wasm migration work for this repository.

The primary track is now `domain-skills/`, not `interaction-skills/`.
Real domain workflows are the best way to prove the Rust guest boundary,
discover missing host operations, and avoid migrating abstract guidance too
early.

`interaction-skills/` remains a secondary extraction track: migrate or refresh
an interaction skill when a real domain workflow proves that the pattern is
stable enough to deserve a typed reusable primitive.

## Global Rules

- [ ] use local browser smokes as the primary acceptance gate
- [ ] keep Browser Use remote verification best-effort only for site-dependent targets
- [ ] prefer browser-first public domains before API-first domains
- [ ] avoid anti-bot/login-heavy domains until the public browser-first queue is stronger
- [ ] only promote reusable typed host operations when a real domain skill proves the need
- [ ] reuse the current smoke pattern everywhere: guest call trace, failed navigation/result details, `page_info` after failure, `page_info` after success

## Done Criteria

A migrated domain skill is only considered done when all of the following are
true:

- [ ] the chosen slice is implemented as a Rust guest or equivalent Rust-owned workflow
- [ ] any missing typed host operation is implemented in the Rust runner/daemon boundary
- [ ] the operation is exposed through `bh-guest-sdk` if guest authors should use it directly
- [ ] a focused local acceptance smoke passes, either against a local browser or as a runner-local `http_get` smoke when no browser is required
- [ ] docs are updated to point at the Rust path as the primary implementation path

## Domain Skill Status

### Completed Browser-First Slices

- [x] `domain-skills/github/scraping.md`
  Rust guest: `rust/guests/rust-github-trending`
  Acceptance: local smoke passes
- [x] `domain-skills/reddit/scraping.md`
  Rust guest: `rust/guests/rust-reddit-post-scrape`
  Acceptance: local smoke passes
- [x] `domain-skills/producthunt/scraping.md`
  Rust guest: `rust/guests/rust-producthunt-homepage`
  Acceptance: local smoke passes
- [x] `domain-skills/letterboxd/scraping.md`
  Rust guest: `rust/guests/rust-letterboxd-popular`
  Acceptance: local popular-page smoke passes; `http_get` film/profile paths still remain dynamic
- [x] `domain-skills/spotify/scraping.md`
  Rust guest: `rust/guests/rust-spotify-search`
  Acceptance: local search-page smoke passes
- [x] `domain-skills/etsy/scraping.md`
  Rust guest: `rust/guests/rust-etsy-search`
  Acceptance: local Etsy search smoke now passes on the current real browser profile

### Completed HTTP-Owned / Mixed Slices

- [x] `domain-skills/metacritic/scraping.md`
  Rust guest: `rust/guests/rust-metacritic-game-scores`
  Acceptance: runner-local `http_get` smoke passes against the public backend API
- [x] `domain-skills/walmart/scraping.md`
  Rust guest: `rust/guests/rust-walmart-search`
  Acceptance: runner-local `http_get` smoke passes against live `__NEXT_DATA__` search HTML
- [x] `domain-skills/tradingview/scraping.md`
  Rust guest: `rust/guests/rust-tradingview-symbol-search`
  Acceptance: runner-local `http_get` smoke passes with the required `Origin` header

The current prioritized domain backlog is complete. The remaining domains below
are intentionally delayed or deprioritized rather than still being part of the
active completion bar.

### Delayed Domains

- [ ] `domain-skills/glassdoor/scraping.md`
- [ ] `domain-skills/g2/scraping.md`
- [ ] `domain-skills/wellfound/scraping.md`
- [ ] `domain-skills/booking-com/scraping.md`

Reason: anti-bot pressure, login walls, or more fragile flows. These are poor
early migration targets.

### Deprioritized For Early Phase 2

- [ ] `domain-skills/hackernews/scraping.md`
- [ ] `domain-skills/dev-to/scraping.md`
- [ ] `domain-skills/archive-org/scraping.md`
- [ ] `domain-skills/stackoverflow/scraping.md`
- [ ] `domain-skills/duckduckgo/scraping.md`
- [ ] `domain-skills/openalex/scraping.md`

Reason: API-first or HTTP-first skills do not stress the browser guest boundary
enough to be high-value early migration work.

## Immediate Domain Tasks

- [x] port the Etsy browser slice and verify it against a local real-browser profile
- [x] move `http_get` into the Rust runner / guest boundary
- [x] port the Metacritic, Walmart, and TradingView public slices on top of runner-owned `http_get`
- [x] mark the current domain-skills-first migration wave complete
- [ ] choose the next wave only after deciding whether to deepen interaction-skill extraction or tackle one delayed anti-bot/login-heavy domain

## Secondary Track: Capability Pull List

Do these only when a domain skill proves the need.

- [ ] typed dialog handling (`accept` / `dismiss` / optional prompt text)
- [x] typed screenshot support
- [ ] typed file upload support
- [ ] typed `dispatch_key` support
- [ ] viewport control helpers (`set_viewport` or equivalent emulation wrapper)
- [ ] typed print-to-PDF support
- [ ] cookie read/write helpers
- [ ] download lifecycle detection/helpers
- [ ] request-side network wait helpers, not only response-side waits
- [ ] low-level drag primitives if current click/scroll/input helpers are not sufficient

## Secondary Track: Interaction Skill Refresh

These are no longer the top-down primary queue. Refresh them when the matching
domain migrations make the guidance concrete enough to stabilize.

### Likely Docs-First Refresh After More Domain Work

- [ ] `interaction-skills/connection.md`
- [ ] `interaction-skills/tabs.md`
- [ ] `interaction-skills/scrolling.md`
- [ ] `interaction-skills/cross-origin-iframes.md`
- [ ] `interaction-skills/iframes.md`
- [ ] `interaction-skills/dropdowns.md`
- [ ] `interaction-skills/shadow-dom.md`
- [x] `interaction-skills/network-requests.md`
  Refreshed around runner-owned `http_get`, `wait_for_response`, and `watch_events`,
  with local acceptance via `scripts/bhrun_response_smoke.py` and
  `scripts/bhrun_watch_events_smoke.py`
- [x] `interaction-skills/screenshots.md`
  Refreshed around `bhrun screenshot`, `bh_guest_sdk::screenshot(full)`, and
  local acceptance via `scripts/bhrun_screenshot_smoke.py`

### Interaction Skills Waiting On New Typed Host Work

- [ ] `interaction-skills/dialogs.md`
- [ ] `interaction-skills/uploads.md`
- [ ] `interaction-skills/viewport.md`
- [ ] `interaction-skills/print-as-pdf.md`
- [ ] `interaction-skills/downloads.md`
- [ ] `interaction-skills/cookies.md`
- [ ] `interaction-skills/drag-and-drop.md`

### Special Case

- [ ] `interaction-skills/profile-sync.md`
  Treat this primarily as Phase 1 admin/control-plane cleanup and verification,
  not as a guest-boundary migration.

## First Concrete Tasks

If work resumes from the top of the backlog, do these first:

- [ ] port the Etsy slice
- [ ] add the local Etsy smoke
- [ ] verify it locally
- [ ] only then pull the first missing reusable primitive into the interaction-skill track
