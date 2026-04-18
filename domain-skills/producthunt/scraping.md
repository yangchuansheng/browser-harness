# Product Hunt — Daily Launches & Topic Scraping

`https://www.producthunt.com` — React SPA, requires browser (not `http_get`)

## Important: This site needs a browser

Product Hunt is a fully client-rendered React application. `http_get("https://www.producthunt.com")` returns an HTML shell with almost no product data — the launch list, vote counts, and taglines are all injected by the JS bundle after hydration.

Always use `goto` + `wait_for_load` + a short `wait` for React to hydrate, then `js()` to extract.

```python
goto("https://www.producthunt.com")
wait_for_load()
wait(2)  # let React hydrate product cards
```

---

## Common workflows

### 1. Today's daily leaderboard — top N products

This is the homepage default view. Products are sorted by vote count and rendered in `<li>` elements inside the main feed.

```python
goto("https://www.producthunt.com")
wait_for_load()
wait(2)

products = js("""
(function() {
  // Each product card sits inside a <li> with a data-test attribute
  var items = document.querySelectorAll('li[class*="item"]');

  // Fallback selector if the above is empty — PH uses Tailwind, not stable class names.
  // The most reliable anchor is the section heading that says "Today" or a date.
  if (!items || items.length === 0) {
    items = document.querySelectorAll('section[class*="posts"] li');
  }

  return Array.from(items).slice(0, 20).map(function(li) {
    // Product name: inside an <a> that links to /posts/...
    var nameEl = li.querySelector('a[href*="/posts/"] strong, a[href*="/posts/"] span[class*="title"]');
    if (!nameEl) nameEl = li.querySelector('a[href*="/posts/"]');

    // Tagline: second text block inside the card
    var tagEl  = li.querySelector('a[href*="/posts/"] + * span, [class*="tagline"], [class*="description"]');

    // Vote count: the orange/red upvote button
    var voteEl = li.querySelector('[class*="voteButton"] span, button[class*="vote"] span, [data-test*="vote"] span');

    // PH URL (relative)
    var linkEl = li.querySelector('a[href*="/posts/"]');

    return {
      name:     nameEl ? nameEl.innerText.trim() : null,
      tagline:  tagEl  ? tagEl.innerText.trim()  : null,
      votes:    voteEl ? voteEl.innerText.trim()  : null,
      ph_url:   linkEl ? 'https://www.producthunt.com' + linkEl.getAttribute('href') : null,
    };
  }).filter(function(p) { return p.name; });
})()
""")

print(products[:10])
```

**If the selector returns an empty list:** call `screenshot()` to see the current DOM, then inspect what wraps the product cards. PH's class names are Tailwind hashes that occasionally change on deploys. The most stable fallback is to pivot to `a[href*="/posts/"]` anchors:

```python
products = js("""
(function() {
  var seen = new Set();
  var out  = [];
  document.querySelectorAll('a[href^="/posts/"]').forEach(function(a) {
    var href = a.getAttribute('href');
    if (seen.has(href)) return;
    seen.add(href);

    // Walk up to find the card root
    var card = a.closest('li') || a.closest('article') || a.parentElement;
    var text  = a.innerText.trim().split('\\n');

    out.push({
      name:    text[0] || null,
      tagline: text[1] || null,
      ph_url:  'https://www.producthunt.com' + href,
    });
  });
  return out.filter(function(p) { return p.name && p.name.length > 0; });
})()
""")
```

---

### 2. Scroll to load more products (lazy loading)

The homepage loads ~10 products initially. Scrolling fires an intersection observer that appends more. Repeat until you have enough or hit the end of today's launches.

```python
goto("https://www.producthunt.com")
wait_for_load()
wait(2)

info = page_info()
viewport_h = info["h"]

all_products = []
seen_urls = set()

for batch in range(5):  # 5 scrolls × ~10 products = up to ~50
    # Extract currently visible products
    batch_data = js("""
    (function() {
      var seen = new Set();
      var out  = [];
      document.querySelectorAll('a[href^="/posts/"]').forEach(function(a) {
        var href = a.getAttribute('href');
        if (seen.has(href) || !href.match(/^\\/posts\\/[^/]+$/)) return;
        seen.add(href);
        var card = a.closest('li') || a.parentElement;
        var texts = a.innerText.trim().split('\\n').filter(function(s){ return s.trim(); });
        // Vote button is a sibling of the link
        var voteEl = card ? card.querySelector('button[class*="vote"] span, [class*="voteButton"] span') : null;
        out.push({
          name:    texts[0] || null,
          tagline: texts[1] || null,
          votes:   voteEl ? voteEl.innerText.trim() : null,
          ph_url:  'https://www.producthunt.com' + href,
        });
      });
      return out;
    })()
    """)

    for p in (batch_data or []):
        if p.get("ph_url") and p["ph_url"] not in seen_urls:
            seen_urls.add(p["ph_url"])
            all_products.append(p)

    print(f"Batch {batch+1}: {len(all_products)} total products so far")

    # Scroll toward the bottom of the page to trigger lazy loading
    ph = page_info()["ph"]  # current page height
    scroll(760, viewport_h // 2, dy=-(ph // 3))
    wait(2)  # wait for the next batch to render

print(f"Final: {len(all_products)} products")
```

---

### 3. Topic / category pages

Product Hunt organises products by topic at `/topics/<slug>`. These pages list the highest-ranked products for that topic across all time (not just today). They are also client-rendered — use the browser.

Common topic slugs: `developer-tools`, `ai`, `marketing`, `design-tools`, `productivity`, `open-source`, `chrome-extensions`, `no-code`.

```python
topic = "developer-tools"  # or "ai", "marketing", etc.
goto(f"https://www.producthunt.com/topics/{topic}")
wait_for_load()
wait(2)

products = js("""
(function() {
  var seen = new Set();
  var out  = [];
  document.querySelectorAll('a[href^="/posts/"]').forEach(function(a) {
    var href = a.getAttribute('href');
    if (seen.has(href) || !href.match(/^\\/posts\\/[^/]+$/)) return;
    seen.add(href);
    var card   = a.closest('li') || a.closest('article') || a.parentElement;
    var texts  = a.innerText.trim().split('\\n').filter(function(s){ return s.trim(); });
    var voteEl = card ? card.querySelector('[class*="vote"] span, button span') : null;
    out.push({
      name:    texts[0] || null,
      tagline: texts[1] || null,
      votes:   voteEl ? voteEl.innerText.trim() : null,
      ph_url:  'https://www.producthunt.com' + href,
    });
  });
  return out;
})()
""")

print(products[:20])
```

To scroll and load more results on a topic page (same mechanics as the homepage):

```python
for _ in range(4):
    scroll(760, 400, dy=-1200)
    wait(1.5)
# then re-run the js() extraction above
```

---

### 4. Product detail page — description, pricing, maker info, external URL

The detail page at `https://www.producthunt.com/posts/{slug}` contains the full description, the external website URL, tags, gallery images, and maker profiles.

```python
slug = "cursor-the-ai-code-editor"   # replace with actual slug from the leaderboard
goto(f"https://www.producthunt.com/posts/{slug}")
wait_for_load()
wait(2)

detail = js("""
(function() {
  // External website URL — the "Visit website" button points to /redirect/posts/...
  // which redirects to the real external URL. Grab it from the href directly.
  var visitBtn = document.querySelector('a[href*="/redirect/posts/"], a[href*="website"]');
  var extUrl   = visitBtn ? visitBtn.getAttribute('href') : null;

  // Description block — typically a <div> or <p> under the tagline
  var descEl = document.querySelector('[class*="description"], [class*="about"], section p');
  var desc   = descEl ? descEl.innerText.trim() : null;

  // Tags / topics
  var tags = Array.from(
    document.querySelectorAll('a[href*="/topics/"]')
  ).map(function(a){ return a.innerText.trim(); })
   .filter(function(t){ return t.length > 0 && t.length < 40; });

  // Pricing — if there's a pricing section (not all products show one)
  var pricingEl = document.querySelector('[class*="pricing"], [class*="price"]');
  var pricing   = pricingEl ? pricingEl.innerText.trim() : null;

  // Vote count on the detail page
  var voteEl = document.querySelector('[class*="voteButton"] span, button[class*="vote"] span');

  // Maker info — links to /user/... profiles
  var makers = Array.from(
    document.querySelectorAll('a[href*="/user/"]')
  ).map(function(a){
    return {
      name:     a.innerText.trim(),
      ph_url:   'https://www.producthunt.com' + a.getAttribute('href'),
    };
  }).filter(function(m){ return m.name.length > 0; });

  // Gallery image URLs
  var images = Array.from(
    document.querySelectorAll('img[class*="gallery"], [class*="media"] img, [class*="screenshot"] img')
  ).map(function(img){ return img.src; })
   .filter(function(src){ return src && !src.includes('avatar') && !src.includes('logo'); });

  return {
    external_url: extUrl,
    description:  desc,
    tags:         [...new Set(tags)].slice(0, 10),
    pricing:      pricing,
    votes:        voteEl ? voteEl.innerText.trim() : null,
    makers:       makers.slice(0, 5),
    gallery:      images.slice(0, 8),
  };
})()
""")

print(detail)
```

**Getting the real external URL (not the PH redirect):** PH wraps outbound links in `/redirect/posts/{id}?url=...`. The `url` query param contains the actual destination. Parse it without clicking:

```python
import urllib.parse

redirect_href = detail.get("external_url", "")   # e.g. "/redirect/posts/123?url=https%3A%2F%2F..."
if redirect_href and "url=" in redirect_href:
    qs = urllib.parse.parse_qs(urllib.parse.urlparse(redirect_href).query)
    real_url = qs.get("url", [None])[0]
    print("External site:", real_url)
```

---

### 5. First 20 products with full details (name, website, description, tags, votes, logo)

This combines the leaderboard scroll with per-product detail fetching. It is slow (one `goto` per product) — use only when you genuinely need description + external URL for every product.

```python
goto("https://www.producthunt.com")
wait_for_load()
wait(2)

# Step 1: collect slugs from the leaderboard
slugs = js("""
(function() {
  var seen = new Set();
  var out  = [];
  document.querySelectorAll('a[href^="/posts/"]').forEach(function(a) {
    var href = a.getAttribute('href');
    if (seen.has(href) || !href.match(/^\\/posts\\/[^/]+$/)) return;
    seen.add(href);
    out.push(href);
  });
  return out.slice(0, 20);
})()
""")

# Step 2: visit each product page and extract details
import time, urllib.parse

results = []
for ph_path in slugs:
    goto(f"https://www.producthunt.com{ph_path}")
    wait_for_load()
    wait(1.5)

    row = js("""
    (function() {
      var visitBtn  = document.querySelector('a[href*="/redirect/posts/"]');
      var logoEl    = document.querySelector('img[alt*="logo"], [class*="logo"] img, header img');
      var nameEl    = document.querySelector('h1');
      var taglineEl = document.querySelector('h2, [class*="tagline"]');
      var descEl    = document.querySelector('[class*="description"] p, section > p, [class*="about"] p');
      var voteEl    = document.querySelector('[class*="voteButton"] span, button[class*="vote"] span');
      var tags      = Array.from(document.querySelectorAll('a[href*="/topics/"]'))
                          .map(function(a){ return a.innerText.trim(); })
                          .filter(function(t){ return t && t.length < 40; });
      return {
        name:         nameEl    ? nameEl.innerText.trim()    : null,
        tagline:      taglineEl ? taglineEl.innerText.trim() : null,
        description:  descEl   ? descEl.innerText.trim()    : null,
        votes:        voteEl   ? voteEl.innerText.trim()    : null,
        redirect_url: visitBtn  ? visitBtn.getAttribute('href') : null,
        logo_url:     logoEl   ? logoEl.src : null,
        tags:         [...new Set(tags)].slice(0, 8),
      };
    })()
    """)

    # Resolve external URL from redirect href
    if row and row.get("redirect_url") and "url=" in row["redirect_url"]:
        qs = urllib.parse.parse_qs(urllib.parse.urlparse(row["redirect_url"]).query)
        row["website"] = qs.get("url", [None])[0]
    else:
        row["website"] = None

    row["ph_url"] = f"https://www.producthunt.com{ph_path}"
    results.append(row)
    time.sleep(0.5)  # polite pacing

print(results)
```

---

### 6. GraphQL API (advanced — fragile)

Product Hunt exposes a GraphQL endpoint at `https://www.producthunt.com/frontend/graphql`. It is used by the SPA itself and returns structured JSON without DOM parsing. The catch: it requires a `_ph_puuid` cookie and an `Authorization: Bearer <token>` that is minted client-side and rotates on each session.

**Grab the live token from the running browser session:**

```python
# 1. Make sure producthunt.com is open in the browser
goto("https://www.producthunt.com")
wait_for_load()
wait(2)

# 2. Extract the bearer token from localStorage / window.__APOLLO_STATE__
token = js("""
(function() {
  // PH stores auth in a localStorage key like "ph_access_token" or via Apollo headers
  // Try common locations:
  var t = localStorage.getItem('ph_access_token')
       || localStorage.getItem('access_token');
  if (t) return t;

  // Fallback: intercept from the last XHR — not easy without Network.enable.
  // Return null if not found; use the cookie approach below instead.
  return null;
})()
""")

# 3. Get cookies to send with the request
cookies_raw = js("document.cookie")
# cookies_raw is "key=val; key2=val2; ..."

print("Token:", token)
print("Cookies:", cookies_raw[:200])
```

**Issue a GraphQL query using the token:**

```python
import json

# Today's top posts (posts endpoint)
query = """
query TodaysPosts($first: Int) {
  posts(order: VOTES, first: $first, postedAfter: "today") {
    edges {
      node {
        id
        name
        tagline
        votesCount
        website
        slug
        topics {
          edges { node { name } }
        }
        thumbnail { url }
      }
    }
  }
}
"""

headers = {
    "Content-Type": "application/json",
    "Accept":       "application/json",
    "Origin":       "https://www.producthunt.com",
    "Referer":      "https://www.producthunt.com/",
}
if token:
    headers["Authorization"] = f"Bearer {token}"
if cookies_raw:
    headers["Cookie"] = cookies_raw

body = json.dumps({"query": query, "variables": {"first": 20}}).encode()

# http_get only does GET — use a raw CDP network request for POST
response = js(f"""
(async function() {{
  const r = await fetch('https://www.producthunt.com/frontend/graphql', {{
    method: 'POST',
    headers: {json.dumps(headers)},
    body: JSON.stringify({{query: {json.dumps(query)}, variables: {{first: 20}}}})
  }});
  return await r.text();
}})()
""")

data = json.loads(response)
posts = data["data"]["posts"]["edges"]
for edge in posts:
    node = edge["node"]
    print(node["name"], "|", node["tagline"], "|", node["votesCount"], "|", node["website"])
```

**Why this is fragile:**
- The bearer token changes every session and cannot be hardcoded.
- PH may add CSRF checks (`x-csrf-token`, `x-request-id`) that break the fetch.
- The GraphQL schema has changed several times — field names like `postsConnection` vs `posts` vary by PH's deployment.
- Use DOM scraping as the primary method; treat GraphQL as a faster alternative to verify or enrich data when it works.

---

## Gotchas

- **Products load in batches — scroll and wait between batches.** The first `wait_for_load()` only signals the HTML shell is ready. React renders the first ~10 products, then more are appended as you scroll. Each `scroll()` should be followed by `wait(1.5)` to `wait(2)` to let the fetch-and-render cycle finish.

- **Vote counts update in real-time.** Product Hunt uses WebSockets (Pusher) to push live vote increments. A vote count you read at T+0 may differ from one read at T+30s. Treat them as point-in-time snapshots, not authoritative totals.

- **`/topics/` pages rank by all-time votes, not today.** If the task says "today's top developer tools," use the homepage and filter by topic tags on the detail page — don't use `/topics/developer-tools`, which will return products from past years.

- **The external website URL is never directly in the link text.** PH wraps all outbound links in `/redirect/posts/{id}?url=<encoded>`. Parse the `url=` query param from the redirect `href` attribute — do NOT click it (that opens a new tab and loses context). See the `urllib.parse` snippet in workflow 4.

- **PH uses Tailwind — class names are hashed and not stable across deploys.** Never use a class name like `styles_voteButton__xK9q2` as a selector. Always target structural attributes (`href`, `data-test*`, `aria-*`) or element type + hierarchy. The `a[href^="/posts/"]` anchor is the most stable entry point.

- **Login is not required for reading** — vote counts, product names, taglines, descriptions, and maker profiles are all publicly accessible. Login is required only to upvote, comment, or access private launches.

- **The `#1 product` on the homepage** is ranked by votes within the current day (midnight UTC reset). The ordering is stable by mid-day but can change rapidly in the first few hours after midnight as votes accumulate.

- **Pagination on topic pages** works via infinite scroll, same as the homepage. There is no `?page=N` URL parameter for the browser-rendered view. Scroll to get more.

- **`screenshot()` is your best debugging tool.** If any `js()` extraction returns empty or `None`, always call `screenshot()` before trying alternative selectors — the page may still be loading, a login modal may have appeared, or PH may have A/B-tested a new layout.

- **Date boundary:** PH resets the daily leaderboard at **midnight UTC**, not midnight in the user's local timezone. If you're scraping just after midnight UTC, the new day's list may have zero or very few products.

- **Product Hunt CDN assets** (logos, gallery images) are served from `ph-files.imgix.net`. These are publicly accessible without auth and can be fetched with `http_get`.
