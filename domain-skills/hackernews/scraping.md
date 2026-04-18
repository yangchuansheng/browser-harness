# Hacker News — Scraping Stories, Comments & Search

`https://news.ycombinator.com` — Classic server-rendered HTML tables. No JS required to render content.

## Do this first

**Use HTTP, not the browser.** HN is pure server-rendered HTML — all story data is in the raw source. `http_get` is 10–20x faster than `goto + wait_for_load`, and there is no rate-limited API to worry about for basic reads.

```python
html = http_get("https://news.ycombinator.com")
# Full front-page HTML — stories are in <tr class="athing"> rows
```

Never open the browser for front-page or listing reads. Reserve `goto` for interactive tasks only (e.g. submitting a comment, logging in).

---

## Approach 1 — Front-page and listing pages (JS extraction)

HN's DOM is a plain HTML table. After `http_get`, evaluate JS selectors against the raw HTML via `js()` **if you're already in a browser session**, or parse with a lightweight regex approach for pure-HTTP tasks.

### Using `js()` after `goto` (when a browser session is already open)

```python
goto("https://news.ycombinator.com")
wait_for_load()

stories = js("""
(() => {
  const rows = document.querySelectorAll('tr.athing');
  return Array.from(rows).map(row => {
    const titleAnchor = row.querySelector('span.titleline > a');
    const subRow      = row.nextElementSibling;
    const scoreEl     = subRow && subRow.querySelector('span.score');
    const authorEl    = subRow && subRow.querySelector('a.hnuser');
    const ageEl       = subRow && subRow.querySelector('span.age a');
    const commentsEl  = subRow && [...subRow.querySelectorAll('a')]
                          .find(a => a.textContent.includes('comment'));
    return {
      id:       row.getAttribute('id'),
      title:    titleAnchor ? titleAnchor.textContent.trim() : '',
      url:      titleAnchor ? titleAnchor.href : '',
      points:   scoreEl    ? parseInt(scoreEl.textContent)  : 0,
      author:   authorEl   ? authorEl.textContent.trim()    : '',
      age:      ageEl      ? ageEl.textContent.trim()       : '',
      comments: commentsEl ? parseInt(commentsEl.textContent) : 0,
    };
  });
})()
""")
# returns a list of dicts, one per story (up to 30 per page)
```

### Using pure `http_get` + Python regex (no browser needed)

```python
import re, json

def parse_hn_page(url):
    html = http_get(url)

    # Each story block: <tr class="athing" id="STORY_ID">
    athing_ids = re.findall(r'<tr class="athing" id="(\d+)">', html)

    # Title + URL: <span class="titleline"><a href="URL">TITLE</a>
    title_urls = re.findall(
        r'<span class="titleline"><a href="([^"]*)"[^>]*>([^<]+)</a>', html
    )

    # Points: <span class="score" id="score_STORYID">N points</span>
    scores = re.findall(r'<span class="score"[^>]*>(\d+) points?</span>', html)

    # Author: <a class="hnuser" href="user?id=USERNAME">USERNAME</a>
    authors = re.findall(r'<a class="hnuser"[^>]*>([^<]+)</a>', html)

    # Comment counts: last <a> in subrow, text like "42\xa0comments"
    comments = re.findall(
        r'(\d+)&nbsp;comment', html
    )

    stories = []
    for i, sid in enumerate(athing_ids):
        url_raw, title = title_urls[i] if i < len(title_urls) else ('', '')
        # HN "text posts" have relative URLs like "item?id=..."
        full_url = (
            url_raw if url_raw.startswith('http')
            else f"https://news.ycombinator.com/{url_raw}"
        )
        stories.append({
            'id':       sid,
            'title':    title,
            'url':      full_url,
            'points':   int(scores[i])   if i < len(scores)   else 0,
            'author':   authors[i]       if i < len(authors)   else '',
            'comments': int(comments[i]) if i < len(comments) else 0,
        })
    return stories

top_stories = parse_hn_page("https://news.ycombinator.com")
```

---

## Approach 2 — Algolia HN Search API (best for search + date filtering)

The Algolia API is the fastest way to search HN by keyword, filter by date, or get stories above a points threshold. No HTML parsing needed.

Base URL: `http://hn.algolia.com/api/v1/search`

```python
import json, time

def hn_search(query, tags="story", min_points=0, since_hours=24, num=10):
    """
    Search HN stories via Algolia.
    tags: "story" | "ask_hn" | "show_hn" | "comment" | "poll"
    """
    since_ts = int(time.time()) - since_hours * 3600
    params = (
        f"query={query}"
        f"&tags={tags}"
        f"&numericFilters=points>{min_points},created_at_i>{since_ts}"
        f"&hitsPerPage={num}"
    )
    resp = http_get(f"http://hn.algolia.com/api/v1/search?{params}")
    data = json.loads(resp)
    return [
        {
            'objectID': h['objectID'],           # story ID
            'title':    h.get('title', ''),
            'url':      h.get('url') or f"https://news.ycombinator.com/item?id={h['objectID']}",
            'points':   h.get('points', 0),
            'author':   h.get('author', ''),
            'comments': h.get('num_comments', 0),
            'created':  h.get('created_at', ''),  # ISO 8601 string
            'created_ts': h.get('created_at_i', 0),  # Unix timestamp
        }
        for h in data.get('hits', [])
    ]

# Examples:
results = hn_search("AI agents", since_hours=24, min_points=10, num=20)
results = hn_search("", tags="show_hn", since_hours=48, num=10)  # top Show HN last 48h
results = hn_search("Rust", tags="story", min_points=100, num=5)  # high-signal Rust posts
```

Use `search_by_date` endpoint instead of `search` to sort by recency rather than relevance:

```python
resp = http_get(
    "http://hn.algolia.com/api/v1/search_by_date"
    "?query=python&tags=story&hitsPerPage=10"
)
```

---

## Common workflows

### Top N stories from front page

```python
stories = parse_hn_page("https://news.ycombinator.com")
top3 = stories[:3]
# Each dict: {id, title, url, points, author, comments}
```

### Show HN extraction

```python
stories = parse_hn_page("https://news.ycombinator.com/show")
top10 = stories[:10]
```

### Ask HN extraction

```python
stories = parse_hn_page("https://news.ycombinator.com/ask")
top5 = stories[:5]
```

### Page 2 (stories 31–60)

```python
page2 = parse_hn_page("https://news.ycombinator.com/?p=2")
```

### Monitor for topic in last 24 hours

```python
# Preferred: Algolia API — no scraping, timestamp-accurate
hits = hn_search("postgres", since_hours=24, min_points=0, num=30)
matching = [h for h in hits if h['points'] > 5]  # filter noise
```

### Get comments from a thread

```python
import re

def get_hn_comments(item_id, max_comments=20):
    html = http_get(f"https://news.ycombinator.com/item?id={item_id}")

    # Comment text is inside <span class="commtext ...">
    texts = re.findall(
        r'<span class="commtext[^"]*">(.*?)</span>\s*</div>',
        html, re.DOTALL
    )
    # Strip inline HTML tags for plain text
    def strip_tags(s):
        return re.sub(r'<[^>]+>', '', s).strip()

    # Authors per comment: <a class="hnuser" ...>USERNAME</a>
    authors = re.findall(r'<a class="hnuser"[^>]*>([^<]+)</a>', html)
    # First author is story submitter — skip it for comment authors
    comment_authors = authors[1:]

    comments = []
    for i, text in enumerate(texts[:max_comments]):
        comments.append({
            'author': comment_authors[i] if i < len(comment_authors) else '',
            'text':   strip_tags(text),
        })
    return comments

comments = get_hn_comments("43567890")
```

For deep thread extraction (nested replies, vote counts), use the Algolia Items API — it returns the full thread as JSON:

```python
import json

def get_thread_algolia(item_id):
    resp = http_get(f"http://hn.algolia.com/api/v1/items/{item_id}")
    return json.loads(resp)
    # Top-level keys: id, title, url, points, author, children (list of comment dicts)

thread = get_thread_algolia("43567890")
top_comments = thread.get('children', [])[:10]
```

### Export to CSV

```python
import csv, io

def stories_to_csv(stories):
    buf = io.StringIO()
    w = csv.DictWriter(buf, fieldnames=['title', 'url', 'points', 'author', 'comments'])
    w.writeheader()
    w.writerows(stories)
    return buf.getvalue()

csv_text = stories_to_csv(top10)
# Write to file:
with open("/tmp/hn_show.csv", "w") as f:
    f.write(csv_text)
```

---

## Gotchas

- **30 items per page** — HN hard-caps each listing to 30 stories. Use `?p=2`, `?p=3` etc. to paginate. The next-page link also carries a `?fnid=` token on some endpoints (Best, Active), which changes each page load — safer to use `?p=N` where it works.
- **Story URLs can be relative** — "Ask HN" and "text posts" set `href="item?id=XXXXXXX"` (no leading slash). Always check `url.startswith('http')` and prepend `https://news.ycombinator.com/` if not.
- **Points vs score** — the `<span class="score">` element shows "N points" (plural even for 1). Parse with `int(re.search(r'(\d+)', score_text).group(1))`. Jobs posts and some flagged posts have no score element at all.
- **Comment count text** — uses a non-breaking space (`\xa0` / `&nbsp;`): `"42\xa0comments"`. Use `int(re.search(r'(\d+)', text).group(1))` to parse safely. Posts with zero comments show "discuss" not "0 comments" — handle that case.
- **Algolia timestamps are Unix integers** — `created_at_i` is a Unix timestamp (seconds). `created_at` is ISO 8601. Convert: `datetime.utcfromtimestamp(h['created_at_i'])`.
- **Algolia `url` field is None for text posts** — `h.get('url')` returns `None` for Ask/Show HN posts that have no external link. Always fall back: `h.get('url') or f"https://news.ycombinator.com/item?id={h['objectID']}"`.
- **Rate limiting** — HN has no official rate limit for reads, but aggressive scraping will get you temporarily 503'd. One request per second is safe. For bulk work, the Algolia API is more tolerant than scraping HN directly.
- **Dead/flagged stories** — `[dead]` or `[flagged]` stories appear in the HTML but have no score element and no author link. They show up as empty fields in regex parses — filter with `if s['title']` before returning results.
- **"More" link at page bottom is not reliable for automation** — it uses `?fnid=` tokens that expire. Use explicit `?p=N` pagination instead.
- **Browser not needed** — HN has zero JS-rendered content. Every story, comment, and score is in the initial HTML. Never use `goto + wait_for_load + screenshot` for read-only HN tasks.
