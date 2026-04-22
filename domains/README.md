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
- extra notes can live beside it, such as
  [`github/repo-actions.md`](github/repo-actions.md)
- executable guests still live under [`../rust/guests/`](../rust/guests/) for
  now
- some migrated sites are knowledge-only and do not have a guest crate yet
- `domain-skills/` contains legacy site guides that have not yet been migrated

Currently migrated sites:

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
- [`crossref/skill.md`](crossref/skill.md)
- [`ebay/skill.md`](ebay/skill.md)
- [`stackoverflow/skill.md`](stackoverflow/skill.md)
- [`open-library/skill.md`](open-library/skill.md)
- [`openalex/skill.md`](openalex/skill.md)
- [`pubmed/skill.md`](pubmed/skill.md)
- [`tiktok/skill.md`](tiktok/skill.md)
- [`weather/skill.md`](weather/skill.md)
- [`linkedin/skill.md`](linkedin/skill.md) (stub; guide not written yet)

When adding or updating a migrated site, prefer `domains/<site>/`.
