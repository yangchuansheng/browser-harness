# Domains

`domains/` is the active home for site-specific Browser Harness knowledge.

Target shape:

```text
domains/
  <site>/
    skill.md
    guest/
    fixtures/
```

Current rules:

- `skill.md` is the main site guide
- `skill.md` is the primary artifact; guests are optional
- extra notes can live beside it, such as
  [`github/repo-actions.md`](github/repo-actions.md)
- executable guests still live under [`../rust/guests/`](../rust/guests/) for
  now
- some sites are knowledge-only and do not have a guest crate yet
- all active site-specific knowledge now lives under `domains/`
- code examples may use Python-like helper syntax as pseudocode for harness
  operations, not as a Python runtime requirement

Current sites:

Guest-backed sites:

- [`github/skill.md`](github/skill.md) -> [`../rust/guests/rust-github-trending/`](../rust/guests/rust-github-trending/)
- [`reddit/skill.md`](reddit/skill.md) -> [`../rust/guests/rust-reddit-post-scrape/`](../rust/guests/rust-reddit-post-scrape/)
- [`producthunt/skill.md`](producthunt/skill.md) -> [`../rust/guests/rust-producthunt-homepage/`](../rust/guests/rust-producthunt-homepage/)
- [`letterboxd/skill.md`](letterboxd/skill.md) -> [`../rust/guests/rust-letterboxd-popular/`](../rust/guests/rust-letterboxd-popular/)
- [`spotify/skill.md`](spotify/skill.md) -> [`../rust/guests/rust-spotify-search/`](../rust/guests/rust-spotify-search/)
- [`etsy/skill.md`](etsy/skill.md) -> [`../rust/guests/rust-etsy-search/`](../rust/guests/rust-etsy-search/)
- [`metacritic/skill.md`](metacritic/skill.md) -> [`../rust/guests/rust-metacritic-game-scores/`](../rust/guests/rust-metacritic-game-scores/)
- [`walmart/skill.md`](walmart/skill.md) -> [`../rust/guests/rust-walmart-search/`](../rust/guests/rust-walmart-search/)
- [`tradingview/skill.md`](tradingview/skill.md) -> [`../rust/guests/rust-tradingview-symbol-search/`](../rust/guests/rust-tradingview-symbol-search/)

Knowledge-only sites:

- [`archive-org/skill.md`](archive-org/skill.md)
- [`wayback-machine/skill.md`](wayback-machine/skill.md)
- [`arxiv/skill.md`](arxiv/skill.md)
- [`arxiv-bulk/skill.md`](arxiv-bulk/skill.md)
- [`amazon/skill.md`](amazon/skill.md)
- [`atlas/skill.md`](atlas/skill.md)
- [`booking-com/skill.md`](booking-com/skill.md)
- [`coingecko/skill.md`](coingecko/skill.md)
- [`coinmarketcap/skill.md`](coinmarketcap/skill.md)
- [`capterra/skill.md`](capterra/skill.md)
- [`centilebrain/skill.md`](centilebrain/skill.md)
- [`craigslist/skill.md`](craigslist/skill.md)
- [`crossref/skill.md`](crossref/skill.md)
- [`coursera/skill.md`](coursera/skill.md)
- [`dev-to/skill.md`](dev-to/skill.md)
- [`duckduckgo/skill.md`](duckduckgo/skill.md)
- [`ebay/skill.md`](ebay/skill.md)
- [`eventbrite/skill.md`](eventbrite/skill.md)
- [`facebook/skill.md`](facebook/skill.md)
- [`fred/skill.md`](fred/skill.md)
- [`framer/skill.md`](framer/skill.md)
- [`g2/skill.md`](g2/skill.md)
- [`genius/skill.md`](genius/skill.md)
- [`glassdoor/skill.md`](glassdoor/skill.md)
- [`goodreads/skill.md`](goodreads/skill.md)
- [`gutenberg/skill.md`](gutenberg/skill.md)
- [`hackernews/skill.md`](hackernews/skill.md)
- [`howlongtobeat/skill.md`](howlongtobeat/skill.md)
- [`itch-io/skill.md`](itch-io/skill.md)
- [`job-boards/skill.md`](job-boards/skill.md)
- [`stackoverflow/skill.md`](stackoverflow/skill.md)
- [`musicbrainz/skill.md`](musicbrainz/skill.md)
- [`macrotrends/skill.md`](macrotrends/skill.md)
- [`medium/skill.md`](medium/skill.md)
- [`nasa/skill.md`](nasa/skill.md)
- [`news-aggregation/skill.md`](news-aggregation/skill.md)
- [`open-library/skill.md`](open-library/skill.md)
- [`openalex/skill.md`](openalex/skill.md)
- [`openstreetmap/skill.md`](openstreetmap/skill.md)
- [`package-registries/skill.md`](package-registries/skill.md)
- [`pubmed/skill.md`](pubmed/skill.md)
- [`quora/skill.md`](quora/skill.md)
- [`rawg/skill.md`](rawg/skill.md)
- [`rest-countries/skill.md`](rest-countries/skill.md)
- [`salesforce/skill.md`](salesforce/skill.md)
- [`sec-edgar/skill.md`](sec-edgar/skill.md)
- [`soundcloud/skill.md`](soundcloud/skill.md)
- [`spreadshirt/skill.md`](spreadshirt/skill.md)
- [`tiktok/skill.md`](tiktok/skill.md)
- [`thetechgeeks/skill.md`](thetechgeeks/skill.md)
- [`steam/skill.md`](steam/skill.md)
- [`trello/skill.md`](trello/skill.md)
- [`trustpilot/skill.md`](trustpilot/skill.md)
- [`weather/skill.md`](weather/skill.md)
- [`zillow/skill.md`](zillow/skill.md)
- [`wellfound/skill.md`](wellfound/skill.md)
- [`world-bank/skill.md`](world-bank/skill.md)
- [`linkedin/skill.md`](linkedin/skill.md)

When adding or updating a site, prefer `domains/<site>/`.
