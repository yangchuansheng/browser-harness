# News Aggregation — Multi-Source Fetching

Covers TechCrunch, The Verge, Ars Technica, BBC, Reuters, VentureBeat, and similar editorial news sites. For Hacker News specifically, see `hackernews/scraping.md`.

## Do this first

**Use RSS feeds, not the browser.** Almost every major news publication exposes RSS. A single `http_get` on an RSS URL gives you titles, URLs, summaries, authors, and publish dates as structured XML — no screenshots, no JS rendering, no cookie banners. This is 10–20x faster than browser-based scraping and works reliably even on sites that heavily JS-render their front pages.

```python
# Known RSS feeds (verified 2026-04):
RSS_FEEDS = {
    "techcrunch":    "https://techcrunch.com/feed/",
    "verge":         "https://www.theverge.com/rss/index.xml",
    "ars_technica":  "https://feeds.arstechnica.com/arstechnica/index",
    "bbc":           "http://feeds.bbci.co.uk/news/rss.xml",
    "reuters":       "https://feeds.reuters.com/reuters/topNews",
    "venturebeat":   "https://venturebeat.com/feed/",
}

# Category/topic feeds:
TOPIC_FEEDS = {
    "techcrunch_ai":   "https://techcrunch.com/category/artificial-intelligence/feed/",
    "techcrunch_startups": "https://techcrunch.com/category/startups/feed/",
    "verge_ai":        "https://www.theverge.com/rss/ai-artificial-intelligence/index.xml",
    "verge_tech":      "https://www.theverge.com/rss/tech/index.xml",
    "ars_tech":        "https://feeds.arstechnica.com/arstechnica/technology-lab",
    "ars_science":     "https://feeds.arstechnica.com/arstechnica/science",
    "bbc_tech":        "http://feeds.bbci.co.uk/news/technology/rss.xml",
    "bbc_world":       "http://feeds.bbci.co.uk/news/world/rss.xml",
    "reuters_world":   "https://feeds.reuters.com/reuters/worldnews",
    "reuters_tech":    "https://feeds.reuters.com/reuters/technologynews",
}
```

---

## Parsing RSS — the core utility

RSS is XML. Parse it with Python's built-in `xml.etree.ElementTree` — no third-party libraries needed.

```python
import xml.etree.ElementTree as ET
import re
from email.utils import parsedate_to_datetime   # handles RFC 822 dates
from datetime import datetime, timezone

def parse_rss(xml_text, source_name=""):
    """Parse an RSS 2.0 feed. Returns a list of article dicts."""
    # Some feeds include XML namespace declarations that break plain ET parsing.
    # Strip them first so we can use simple tag names.
    xml_text = re.sub(r' xmlns[^"]*"[^"]*"', '', xml_text)
    xml_text = re.sub(r'<\?xml[^?]*\?>', '', xml_text).strip()

    try:
        root = ET.fromstring(xml_text)
    except ET.ParseError:
        # Malformed XML — last resort, grab titles with regex
        titles = re.findall(r'<title><!\[CDATA\[(.*?)\]\]></title>', xml_text)
        links  = re.findall(r'<link>(https?://[^<]+)</link>', xml_text)
        return [{"title": t, "url": l, "source": source_name}
                for t, l in zip(titles, links)]

    items = root.findall(".//item")  # RSS 2.0
    if not items:
        items = root.findall(".//{http://www.w3.org/2005/Atom}entry")  # Atom

    articles = []
    for item in items:
        def text(tag, default=""):
            el = item.find(tag)
            if el is None:
                return default
            t = el.text or ""
            # Strip CDATA wrapper if present (some feeds inline it as text)
            t = re.sub(r'<!\[CDATA\[(.*?)\]\]>', r'\1', t, flags=re.DOTALL)
            return t.strip()

        title   = text("title")
        url     = text("link") or text("guid")
        summary = text("description") or text("summary") or text("{http://www.w3.org/2005/Atom}summary")
        author  = text("author") or text("{http://purl.org/dc/elements/1.1/}creator")
        pub_raw = text("pubDate") or text("published") or text("updated")

        # Normalise publish date to a UTC datetime (or None)
        pub_dt = _parse_date(pub_raw)

        if not title and not url:
            continue

        articles.append({
            "title":   title,
            "url":     url,
            "summary": re.sub(r'<[^>]+>', '', summary).strip(),  # strip HTML tags
            "author":  author,
            "source":  source_name,
            "pub_raw": pub_raw,
            "pub_dt":  pub_dt,
        })

    return articles


def _parse_date(s):
    """Try multiple date formats. Returns a UTC-aware datetime or None."""
    if not s:
        return None
    s = s.strip()
    # RFC 822 (most RSS feeds): "Fri, 18 Apr 2026 10:30:00 +0000"
    try:
        return parsedate_to_datetime(s).astimezone(timezone.utc)
    except Exception:
        pass
    # ISO 8601 (Atom, some modern feeds): "2026-04-18T10:30:00Z"
    for fmt in ("%Y-%m-%dT%H:%M:%SZ", "%Y-%m-%dT%H:%M:%S%z", "%Y-%m-%d"):
        try:
            dt = datetime.strptime(s, fmt)
            if dt.tzinfo is None:
                dt = dt.replace(tzinfo=timezone.utc)
            return dt.astimezone(timezone.utc)
        except ValueError:
            continue
    return None
```

---

## Parallel fetch — multiple feeds simultaneously

Use `ThreadPoolExecutor` to fetch all feeds in parallel. With 6 feeds each taking ~300ms, sequential fetching takes ~1.8s; parallel takes ~350ms.

```python
from concurrent.futures import ThreadPoolExecutor, as_completed

def fetch_feed(name, url):
    """Fetch and parse one RSS feed. Returns (name, articles) or (name, []) on error."""
    try:
        xml = http_get(url, headers={"Accept": "application/rss+xml, application/xml, text/xml"})
        return name, parse_rss(xml, source_name=name)
    except Exception as e:
        print(f"[{name}] fetch failed: {e}")
        return name, []


def fetch_all_feeds(feed_dict, max_workers=8):
    """Fetch multiple RSS feeds in parallel. Returns merged list of articles."""
    all_articles = []
    with ThreadPoolExecutor(max_workers=max_workers) as ex:
        futures = {ex.submit(fetch_feed, name, url): name
                   for name, url in feed_dict.items()}
        for fut in as_completed(futures):
            name, articles = fut.result()
            all_articles.extend(articles)
    return all_articles


# Example: top AI news across TechCrunch + The Verge + Ars Technica
articles = fetch_all_feeds({
    "techcrunch_ai": TOPIC_FEEDS["techcrunch_ai"],
    "verge_ai":      TOPIC_FEEDS["verge_ai"],
    "ars_tech":      TOPIC_FEEDS["ars_tech"],
})
print(f"Fetched {len(articles)} articles from 3 sources")
```

---

## Common workflows

### Fetch top N articles from a single site

```python
xml = http_get("https://techcrunch.com/feed/")
articles = parse_rss(xml, source_name="techcrunch")
top5 = articles[:5]
for a in top5:
    print(f"[{a['source']}] {a['title']}")
    print(f"  {a['url']}")
    print(f"  {a['pub_raw']}")
```

### Multi-source aggregation with deduplication

Different feeds sometimes carry the same wire story (e.g. Reuters articles reprinted elsewhere). Dedup by URL before presenting results.

```python
def aggregate_and_dedup(feed_dict, max_per_source=None):
    """Fetch feeds, dedup by URL, return sorted by publish date (newest first)."""
    all_articles = fetch_all_feeds(feed_dict)

    # Dedup by URL — keep first occurrence (arbitrary ordering from parallel fetch)
    seen_urls = set()
    unique = []
    for a in all_articles:
        url = a["url"].rstrip("/")   # normalise trailing slash
        if url and url not in seen_urls:
            seen_urls.add(url)
            unique.append(a)

    # Sort: articles with a known pub_dt first (newest), unknown dates last
    def sort_key(a):
        dt = a.get("pub_dt")
        return dt if dt else datetime.min.replace(tzinfo=timezone.utc)

    unique.sort(key=sort_key, reverse=True)

    if max_per_source:
        # Cap per-source contribution so no single feed dominates
        counts = {}
        capped = []
        for a in unique:
            src = a["source"]
            counts[src] = counts.get(src, 0)
            if counts[src] < max_per_source:
                capped.append(a)
                counts[src] += 1
        return capped

    return unique


# Top 5 AI stories across 3 sources in last 24 hours
from datetime import timedelta

results = aggregate_and_dedup({
    "techcrunch_ai": TOPIC_FEEDS["techcrunch_ai"],
    "verge_ai":      TOPIC_FEEDS["verge_ai"],
    "ars_tech":      TOPIC_FEEDS["ars_tech"],
}, max_per_source=10)

cutoff = datetime.now(timezone.utc) - timedelta(hours=24)
recent = [a for a in results if a["pub_dt"] and a["pub_dt"] >= cutoff]
print(f"Found {len(recent)} articles in last 24h")
for a in recent[:5]:
    print(f"\n[{a['source']}] {a['title']}")
    print(f"  Published: {a['pub_dt'].strftime('%Y-%m-%d %H:%M UTC')}")
    print(f"  URL: {a['url']}")
    print(f"  Summary: {a['summary'][:200]}")
```

### Filter by keyword in title or summary

```python
def filter_by_keyword(articles, keywords, case_sensitive=False):
    """Return articles where any keyword appears in title or summary."""
    results = []
    for a in articles:
        haystack = a["title"] + " " + a["summary"]
        if not case_sensitive:
            haystack = haystack.lower()
            checks = [k.lower() for k in keywords]
        else:
            checks = keywords
        if any(k in haystack for k in checks):
            results.append(a)
    return results


# All articles mentioning GPT-4, Claude, or Gemini from the last 48 hours
all_articles = fetch_all_feeds(RSS_FEEDS)
cutoff = datetime.now(timezone.utc) - timedelta(hours=48)
recent = [a for a in all_articles if a["pub_dt"] and a["pub_dt"] >= cutoff]
ai_articles = filter_by_keyword(recent, ["GPT-4", "Claude", "Gemini", "LLM", "AI model"])
for a in ai_articles:
    print(f"[{a['source']}] {a['title']}")
```

### Filter by publish date window

```python
def filter_by_age(articles, hours=24):
    """Return articles published within the last N hours. Articles with no date are excluded."""
    cutoff = datetime.now(timezone.utc) - timedelta(hours=hours)
    return [a for a in articles if a["pub_dt"] and a["pub_dt"] >= cutoff]


# "Top tech news from today" — all feeds, last 24 hours
all_tech = fetch_all_feeds({
    "techcrunch": RSS_FEEDS["techcrunch"],
    "verge":      RSS_FEEDS["verge"],
    "ars":        RSS_FEEDS["ars_technica"],
})
today = filter_by_age(all_tech, hours=24)
today.sort(key=lambda a: a["pub_dt"], reverse=True)
print(f"Today: {len(today)} articles")
```

### Search for a company or topic across all sources

```python
def company_news(company_name, hours=168, feeds=None):
    """
    Get all articles mentioning `company_name` from the last N hours (default: 7 days).
    Uses general feeds — for better coverage, also search Algolia HN (see hackernews/scraping.md).
    """
    feeds = feeds or RSS_FEEDS
    all_articles = fetch_all_feeds(feeds)
    recent = filter_by_age(all_articles, hours=hours)
    matches = filter_by_keyword(recent, [company_name])
    matches.sort(key=lambda a: a["pub_dt"], reverse=True)
    return matches


# "Get all articles about OpenAI from the last week"
openai_news = company_news("OpenAI", hours=168)
for a in openai_news:
    print(f"[{a['pub_dt'].strftime('%m-%d')}] [{a['source']}] {a['title']}")
```

### Extract full article text from a URL

RSS feeds truncate body text (often 50–200 words max). To get full text, follow the article URL. Works on sites without paywalls.

```python
def fetch_article_text(url):
    """
    Fetch full article text from a URL via HTTP (no browser).
    Returns plain text, stripping nav/footer boilerplate.
    Works for: TechCrunch, Ars Technica, Reuters, VentureBeat, BBC.
    Does NOT work for: FT, NYT, Bloomberg, WSJ (paywalls), The Verge (JS-rendered body).
    """
    html = http_get(url)

    # Extract <article> content if present (most modern sites)
    article_match = re.search(r'<article[^>]*>(.*?)</article>', html, re.DOTALL)
    if article_match:
        body_html = article_match.group(1)
    else:
        # Fall back to <main>
        main_match = re.search(r'<main[^>]*>(.*?)</main>', html, re.DOTALL)
        body_html = main_match.group(1) if main_match else html

    # Strip scripts, styles, and HTML tags
    body_html = re.sub(r'<(script|style)[^>]*>.*?</(script|style)>', '', body_html, flags=re.DOTALL)
    text = re.sub(r'<[^>]+>', ' ', body_html)
    text = re.sub(r'\s+', ' ', text).strip()

    return text


# Get full text of the top TechCrunch AI article
xml = http_get(TOPIC_FEEDS["techcrunch_ai"])
articles = parse_rss(xml, "techcrunch")
if articles:
    full_text = fetch_article_text(articles[0]["url"])
    print(full_text[:1000])
```

### Scrape The Verge front page (all articles, title + author + timestamp + URL)

The Verge's front page is JS-rendered, so `http_get` returns a skeleton. Use the browser. But note: the RSS feed is faster if you just need the top articles.

```python
# Option A — RSS (fast, no browser, top articles only)
xml = http_get("https://www.theverge.com/rss/index.xml")
articles = parse_rss(xml, "verge")
# Returns up to 20 articles with title, URL, summary, pub_dt

# Option B — Browser (when you need author bylines or full front-page layout)
goto("https://www.theverge.com")
wait_for_load()
_dismiss_consent_banner()   # see Cookie/Consent section below

articles = js("""
(() => {
  const cards = document.querySelectorAll('h2 a, h3 a');
  const seen = new Set();
  const out = [];
  for (const a of cards) {
    const url = a.href;
    if (!url || seen.has(url)) continue;
    seen.add(url);
    const title = a.textContent.trim();
    if (!title) continue;
    // Author is often in a sibling or parent context — walk up
    const article = a.closest('article') || a.closest('[data-chorus-optimize-field]');
    const authorEl = article && article.querySelector('a[data-analytics-link="author"]');
    const timeEl   = article && article.querySelector('time');
    out.push({
      title:     title,
      url:       url,
      author:    authorEl ? authorEl.textContent.trim() : '',
      timestamp: timeEl   ? timeEl.getAttribute('datetime') : '',
    });
    if (out.length >= 30) break;
  }
  return out;
})()
""")
```

---

## Cookie / consent banner handling

BBC, The Verge, and all EU-facing properties show GDPR consent banners on first visit. The banner intercepts clicks — dismiss it before any other interaction.

### Generic approach (works on most sites)

```python
def _dismiss_consent_banner(wait_seconds=2.0):
    """
    Try to dismiss a cookie/consent banner by clicking 'Accept all', 'Accept', or 'Agree' buttons.
    Call after goto() + wait_for_load(). Safe to call even if no banner is present.
    """
    wait(wait_seconds)   # banners often inject asynchronously

    dismissed = js("""
    (() => {
      const phrases = [
        'accept all', 'accept cookies', 'agree to all', 'i agree',
        'allow all', 'allow cookies', 'ok', 'got it',
      ];
      const els = [
        ...document.querySelectorAll('button'),
        ...document.querySelectorAll('a[role="button"]'),
        ...document.querySelectorAll('[class*="consent"] button'),
        ...document.querySelectorAll('[id*="consent"] button'),
        ...document.querySelectorAll('[class*="cookie"] button'),
        ...document.querySelectorAll('[id*="cookie"] button'),
        ...document.querySelectorAll('[class*="gdpr"] button'),
        ...document.querySelectorAll('[class*="banner"] button'),
      ];
      for (const el of els) {
        const text = (el.textContent || el.value || '').toLowerCase().trim();
        if (phrases.some(p => text.includes(p))) {
          el.click();
          return el.textContent.trim();
        }
      }
      return null;
    })()
    """)

    if dismissed:
        wait(1.0)  # let the banner animate out
    return dismissed
```

### BBC-specific consent

BBC uses a dedicated consent domain (`bbc.co.uk/usp/prd/gdpr`). The consent banner renders inside the page as a full-screen overlay — a regular `button` click via JS usually works. If the JS click doesn't register, fall back to a coordinate click after taking a screenshot to locate the button.

```python
def _dismiss_bbc_consent():
    """Dismiss the BBC consent overlay. Call after goto() + wait_for_load()."""
    wait(2.0)
    # Try JS click first
    result = js("""
    (() => {
      // BBC consent button selector (stable as of 2026-04)
      const btn = document.querySelector('button[data-testid="accept-button"]')
               || document.querySelector('.tp-close')
               || document.querySelector('[class*="ConsentBanner"] button');
      if (btn) { btn.click(); return true; }
      // Fallback: any button with 'Yes' or 'Accept' in a modal/dialog
      for (const b of document.querySelectorAll('dialog button, [role="dialog"] button')) {
        if (/yes|accept|agree/i.test(b.textContent)) { b.click(); return true; }
      }
      return false;
    })()
    """)
    if not result:
        # Fallback: screenshot and coordinate-click the accept button
        screenshot("/tmp/bbc_banner.png")
        # Typical BBC accept button is near top-center on a full-screen overlay
        click(760, 450)
    wait(1.0)
```

**BBC domain note:** `bbc.co.uk/news` serves UK content; `bbc.com/news` serves international. They are different editorial selections. The RSS feed (`feeds.bbci.co.uk`) gives a merged top-stories view. After you dismiss the consent banner once, BBC sets a cookie that persists for the session — subsequent `goto` calls on the same session won't show it again.

---

## When browser IS needed vs pure HTTP

| Task | Use |
|---|---|
| TechCrunch articles (listing + metadata) | RSS `http_get` |
| TechCrunch full article body | `http_get(article_url)` — clean HTML |
| The Verge articles (listing + metadata) | RSS `http_get` |
| The Verge full article body | Browser — body is JS-rendered after hydration |
| Ars Technica articles | RSS `http_get` |
| Ars Technica full article body | `http_get(article_url)` — full HTML |
| BBC articles (listing + metadata) | RSS `http_get` |
| BBC full article body | `http_get(article_url)` usually works |
| Reuters articles | RSS `http_get` |
| Reuters full article body | `http_get(article_url)` — clean HTML |
| VentureBeat articles | RSS `http_get` |
| FT / NYT / Bloomberg / WSJ | Do not use — paywalled |
| "Get the lead article link from bbc.co.uk/news right now" | Browser (need live DOM, not cached RSS) |
| Search results / filtered views with JS | Browser |
| Pagination beyond first RSS page | Browser or site-specific API |

---

## Site-specific notes

### TechCrunch

- General feed: `https://techcrunch.com/feed/` (latest across all categories)
- Category feeds: `https://techcrunch.com/category/<slug>/feed/`
  - AI: `/category/artificial-intelligence/feed/`
  - Startups: `/category/startups/feed/`
  - Venture: `/category/venture/feed/`
  - Security: `/category/security/feed/`
- RSS returns full article summaries (2–4 sentences). No paywall on article pages.
- Author names are in `<dc:creator>` not `<author>` — already handled by `parse_rss`.

### The Verge

- General feed: `https://www.theverge.com/rss/index.xml`
- Topic feeds: `https://www.theverge.com/rss/<topic>/index.xml`
  - AI: `rss/ai-artificial-intelligence/index.xml`
  - Tech: `rss/tech/index.xml`
  - Science: `rss/science/index.xml`
  - Games: `rss/games/index.xml`
- RSS is Atom format (not RSS 2.0) — `parse_rss` handles both.
- Full article body is JS-rendered (React/Chorus). `http_get` on article URLs gives you a skeleton. Use the browser or accept the RSS summary.

### Ars Technica

- General feed: `https://feeds.arstechnica.com/arstechnica/index`
- Topic feeds: `https://feeds.arstechnica.com/arstechnica/<section>`
  - Technology: `technology-lab`
  - Science: `science`
  - Gaming: `gaming`
  - Policy: `tech-policy`
  - Security: `security`
- RSS includes article summaries (first 1–2 paragraphs). Article pages are clean HTML.
- No consent banner in most regions. No paywall.

### BBC

- Top stories: `http://feeds.bbci.co.uk/news/rss.xml`
- World: `http://feeds.bbci.co.uk/news/world/rss.xml`
- Technology: `http://feeds.bbci.co.uk/news/technology/rss.xml`
- Science: `http://feeds.bbci.co.uk/news/science_and_environment/rss.xml`
- RSS descriptions are short (1 sentence). Article HTML is clean and readable.
- Consent banner shows on first browser visit — see BBC-specific handling above.
- `bbc.co.uk/news` = UK-focused editorial; `bbc.com/news` = international. Geo-varies on some stories.

### Reuters

- Top news: `https://feeds.reuters.com/reuters/topNews`
- World: `https://feeds.reuters.com/reuters/worldnews`
- Technology: `https://feeds.reuters.com/reuters/technologynews`
- Business: `https://feeds.reuters.com/reuters/businessNews`
- Article pages are clean HTML with no paywall for standard articles.
- Reuters wire stories are often reproduced verbatim on BBC, AP News, etc. — dedup by URL is important when mixing Reuters with other feeds.

### VentureBeat

- General feed: `https://venturebeat.com/feed/`
- AI feed: `https://venturebeat.com/category/ai/feed/`
- RSS summaries are generous (3–5 sentences). Full articles are clean HTML.
- Some articles behind a "VB Transform" event sign-up modal — usually dismissible, but the article body is present in the raw HTML.

---

## Putting it all together — complete task examples

### "Get top 5 AI stories in last 24 hours from TechCrunch, The Verge, VentureBeat"

```python
import xml.etree.ElementTree as ET
import re
from concurrent.futures import ThreadPoolExecutor, as_completed
from datetime import datetime, timezone, timedelta
from email.utils import parsedate_to_datetime

# (paste parse_rss, _parse_date, fetch_feed, fetch_all_feeds, filter_by_age from above)

feeds = {
    "techcrunch_ai": "https://techcrunch.com/category/artificial-intelligence/feed/",
    "verge_ai":      "https://www.theverge.com/rss/ai-artificial-intelligence/index.xml",
    "venturebeat_ai":"https://venturebeat.com/category/ai/feed/",
}

all_articles = fetch_all_feeds(feeds)
last_24h = filter_by_age(all_articles, hours=24)
last_24h.sort(key=lambda a: a["pub_dt"], reverse=True)
top5 = last_24h[:5]

for a in top5:
    print(f"\nTitle:   {a['title']}")
    print(f"Source:  {a['source']}")
    print(f"URL:     {a['url']}")
    print(f"Date:    {a['pub_dt'].strftime('%Y-%m-%d %H:%M UTC') if a['pub_dt'] else 'unknown'}")
    print(f"Summary: {a['summary'][:300]}")
```

### "Visit BBC World, dismiss cookie banner, get first lead article link"

```python
goto("https://www.bbc.co.uk/news")
wait_for_load()
_dismiss_bbc_consent()  # must run before any other interaction

# Get lead article link from DOM (more reliable than RSS for "right now" front page)
lead_url = js("""
(() => {
  // BBC lead story is usually an <a> inside the first <article> or [data-testid="dundee-card"]
  const selectors = [
    'article h3 a',
    '[data-testid="dundee-card"] a',
    'h3 a[href*="/news/"]',
    'h2 a[href*="/news/"]',
  ];
  for (const sel of selectors) {
    const el = document.querySelector(sel);
    if (el && el.href && !el.href.includes('#')) return el.href;
  }
  return null;
})()
""")
print("Lead article:", lead_url)
```

### "Get today's top tech news from TechCrunch, Ars Technica, and The Verge"

```python
feeds = {
    "techcrunch": "https://techcrunch.com/feed/",
    "ars":        "https://feeds.arstechnica.com/arstechnica/index",
    "verge":      "https://www.theverge.com/rss/index.xml",
}
all_articles = fetch_all_feeds(feeds)
today = filter_by_age(all_articles, hours=24)
today.sort(key=lambda a: a["pub_dt"], reverse=True)

print(f"Today's top tech news ({len(today)} articles):\n")
for i, a in enumerate(today[:15], 1):
    ts = a["pub_dt"].strftime("%H:%M UTC") if a["pub_dt"] else "?"
    print(f"{i:2}. [{a['source']:12}] {ts}  {a['title']}")
```

### "Extract AI/ML news from the last 48 hours across multiple tech publications"

```python
ai_feeds = {
    "techcrunch_ai":   "https://techcrunch.com/category/artificial-intelligence/feed/",
    "verge_ai":        "https://www.theverge.com/rss/ai-artificial-intelligence/index.xml",
    "venturebeat_ai":  "https://venturebeat.com/category/ai/feed/",
    "ars_tech":        "https://feeds.arstechnica.com/arstechnica/technology-lab",
}
# Also pull from general tech feeds and keyword-filter
general_feeds = {
    "bbc_tech":    "http://feeds.bbci.co.uk/news/technology/rss.xml",
    "reuters_tech":"https://feeds.reuters.com/reuters/technologynews",
}

dedicated = fetch_all_feeds(ai_feeds)
general   = fetch_all_feeds(general_feeds)
ai_keywords = ["AI", "machine learning", "LLM", "neural network", "GPT", "Claude",
               "Gemini", "artificial intelligence", "deep learning", "model", "agent"]
filtered_general = filter_by_keyword(general, ai_keywords)

all_ai = dedicated + filtered_general
last_48h = filter_by_age(all_ai, hours=48)

# Dedup by URL
seen = set()
deduped = []
for a in last_48h:
    u = a["url"].rstrip("/")
    if u not in seen:
        seen.add(u)
        deduped.append(a)

deduped.sort(key=lambda a: a["pub_dt"], reverse=True)
print(f"Found {len(deduped)} unique AI/ML articles in last 48h")
```

---

## Gotchas

- **User-Agent required.** Some CDNs (Cloudflare on VentureBeat, Fastly on some Reuters endpoints) return 403 or 429 to Python's default `urllib` agent. `http_get` in this harness already sends `Mozilla/5.0` — don't override it with an empty string. If you hit a 403, try adding an `Accept` header: `{"Accept": "application/rss+xml, text/xml"}`.

- **Publish date formats vary wildly.** RFC 822 (`Fri, 18 Apr 2026 10:30:00 +0000`) is most common in RSS 2.0. Atom feeds use ISO 8601 (`2026-04-18T10:30:00Z`). BBC sometimes omits the seconds. Some feeds use timezone abbreviations (`EST`, `PST`) that are not valid RFC 822 but `email.utils.parsedate_to_datetime` handles them. The `_parse_date` helper above handles all known variants.

- **RSS truncates article body.** RSS `<description>` is at most a few sentences. For full text, `http_get(article_url)` — but this doubles request count. Batch with `ThreadPoolExecutor` if you need full text for many articles.

- **Paywalls.** Do not attempt to scrape FT, NYT, Bloomberg, or WSJ without a subscription. Their articles are behind hard paywalls and bot-detection. Stick to the sources listed above which are freely readable.

- **Cookie consent intercepts clicks.** If you do need to use the browser on BBC or The Verge, always call the dismiss helper before interacting with any link or button. If you click before the banner is dismissed, the click goes to the banner overlay, not the page.

- **BBC geo-serving.** `bbc.co.uk/news` and `bbc.com/news` serve different story selections. The RSS feed `feeds.bbci.co.uk/news/rss.xml` is a merged view and less affected by geo. If a user asks for "BBC World News", use `http://feeds.bbci.co.uk/news/world/rss.xml`.

- **The Verge full article body requires the browser.** Unlike TechCrunch and Ars, The Verge renders article text client-side via React hydration. `http_get(article_url)` gives you only the title and metadata, not the body paragraphs. Accept the RSS summary (which is 2–4 sentences) or use `goto` + `wait_for_load` + `js(...)` to extract the body.

- **Reuters feed freshness.** Reuters top-news feed updates frequently (every few minutes). For real-time news, Reuters is often 10–30 minutes ahead of other outlets.

- **Atom vs RSS 2.0 namespaces.** The Verge and some modern feeds use Atom (`<feed>` root, `<entry>` items). The `parse_rss` helper above detects and handles both formats. If you write your own parser, check the root tag: `feed` = Atom, `rss` = RSS 2.0.

- **`<guid>` vs `<link>`.** In some feeds (particularly Reuters), `<link>` is the feed URL itself and `<guid>` carries the article URL. The `parse_rss` helper tries `link` first and falls back to `guid`. Always verify the URL points to an article, not the feed root.

- **Relative URLs in RSS.** Uncommon but it happens on some smaller publications. Always check `url.startswith('http')` after parsing and prepend the base domain if needed.

- **XML namespace stripping.** Some feeds include namespace declarations like `xmlns:dc="http://purl.org/dc/elements/1.1/"` which cause `ET.fromstring` to require namespace-prefixed tag names. The `re.sub` at the top of `parse_rss` strips these so plain tag names like `dc:creator` are parsed as `"dc:creator"` (colon in tag name) — which ET handles fine.

- **"Relative time" strings.** Some sites (Medium, Substack) put "2 hours ago" in the `<pubDate>` field instead of an absolute timestamp. `_parse_date` will return `None` for these — treat `pub_dt=None` as "recently published" or use the article's position in the feed (item 0 = newest).
