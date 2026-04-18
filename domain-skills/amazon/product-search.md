# Amazon — Product Search & Data Extraction

`https://www.amazon.com` (also `.co.uk`, `.fr`, `.de`, `.co.jp`, `.ca`, `.com.au`)

---

## Do this first: construct URLs directly, don't click the search box

Amazon search URLs are stable and predictable. Build them directly — it's faster and avoids bot-detection on the home page UI.

```python
query = "mechanical keyboard"
goto(f"https://www.amazon.com/s?k={query.replace(' ', '+')}")
wait_for_load()
```

For product detail pages, go straight to the `/dp/` URL:

```python
asin = "B08N5WRWNW"
goto(f"https://www.amazon.com/dp/{asin}")
wait_for_load()
```

---

## URL patterns

| Goal | URL pattern |
|---|---|
| Keyword search | `/s?k=wireless+earbuds` |
| Search in department | `/s?k=chair&i=garden` (i= is the department node) |
| Price range | `/s?k=standing+desk&rh=p_36%3A1000-30000` (cents, e.g. 1000=\$10, 30000=\$300) |
| Min rating filter | `/s?k=headphones&rh=p_72%3A1248957011` (4+ stars node) |
| Prime-only | `/s?k=coffee+maker&rh=p_85%3A2470955011` |
| Combine filters | `/s?k=standing+desk&rh=p_36%3A1000-30000%2Cp_72%3A1248957011` |
| Product detail | `/dp/{ASIN}` |
| Best Sellers (root) | `/Best-Sellers/zgbs/` |
| Best Sellers (category) | `/Best-Sellers/zgbs/electronics/` |
| Best Sellers (subcategory) | `/Best-Sellers/zgbs/electronics/172541/` (node ID) |

Price range encoding: multiply dollar amount by 100 and encode as `p_36%3A{min}-{max}`. Example: \$50–\$300 → `p_36%3A5000-30000`.

---

## Workflow 1: Search results extraction

### Reliable extraction via `data-asin`

Amazon marks every real result card with `data-asin`. Sponsored results get `data-component-type="sp-sponsored-result"` — filter them out.

```python
import json

goto("https://www.amazon.com/s?k=mechanical+keyboard")
wait_for_load()
wait(2)  # let lazy-loaded price data settle

results = js("""
(function() {
  var cards = document.querySelectorAll(
    '[data-component-type="s-search-result"]:not([data-component-type="sp-sponsored-result"])'
  );
  var out = [];
  for (var i = 0; i < Math.min(cards.length, 10); i++) {
    var c = cards[i];
    var asin = c.getAttribute('data-asin') || '';
    var titleEl = c.querySelector('h2 a span, h2 span');
    var title = titleEl ? titleEl.innerText.trim() : '';
    // Price: whole + fraction
    var whole = c.querySelector('.a-price-whole');
    var frac  = c.querySelector('.a-price-fraction');
    var price = (whole && frac)
      ? ('$' + whole.innerText.replace(',','').replace('.','') + '.' + frac.innerText)
      : (c.querySelector('.a-price .a-offscreen') || {innerText:''}).innerText.trim();
    var ratingEl = c.querySelector('i.a-icon-star-small span, i[class*="a-star"] span');
    var rating = ratingEl ? ratingEl.innerText.trim() : '';
    var reviewEl = c.querySelector('[aria-label*="stars"] + span, .a-size-base.s-underline-text');
    var reviews = reviewEl ? reviewEl.innerText.replace(/,/g,'').trim() : '';
    var prime = !!c.querySelector('[aria-label="Amazon Prime"], .s-prime');
    out.push({asin, title, price, rating, reviews, prime});
  }
  return JSON.stringify(out);
})()
""")

products = json.loads(results)
for p in products:
    print(p)
```

### ASIN from URL (search results page)

Each result card's "a" link encodes the ASIN:

```python
asins = js("""
JSON.stringify(
  Array.from(document.querySelectorAll('[data-asin]'))
    .map(el => el.getAttribute('data-asin'))
    .filter(a => a && a.length === 10)
    .filter((a, i, arr) => arr.indexOf(a) === i)  // dedupe
)
""")
# e.g. ["B09XLNBM3B", "B07WRXDH2P", ...]
```

---

## Workflow 2: Product detail page extraction (`/dp/{ASIN}`)

```python
import json, re

asin = "B07XJ8C8F7"
goto(f"https://www.amazon.com/dp/{asin}")
wait_for_load()
wait(2)

detail = js("""
(function() {
  // Title
  var titleEl = document.getElementById('productTitle');
  var title = titleEl ? titleEl.innerText.trim() : '';

  // Price — multiple possible locations
  var priceEl = (
    document.getElementById('priceblock_ourprice') ||
    document.getElementById('priceblock_dealprice') ||
    document.querySelector('.a-price .a-offscreen') ||
    document.querySelector('#corePriceDisplay_desktop_feature_div .a-price .a-offscreen') ||
    document.querySelector('#apex_offerDisplay_desktop .a-price .a-offscreen')
  );
  var price = priceEl ? priceEl.innerText.trim() : '';

  // Rating
  var ratingEl = document.getElementById('acrPopover');
  var rating = ratingEl ? (ratingEl.getAttribute('title') || '').trim() : '';

  // Review count
  var reviewEl = document.getElementById('acrCustomerReviewText');
  var reviews = reviewEl ? reviewEl.innerText.replace(/[^0-9]/g,'') : '';

  // Availability
  var availEl = document.getElementById('availability');
  var availability = availEl ? availEl.innerText.trim() : '';

  // Bullet points (feature list)
  var bullets = Array.from(document.querySelectorAll('#feature-bullets li span.a-list-item'))
    .map(el => el.innerText.trim())
    .filter(t => t.length > 0);

  return JSON.stringify({title, price, rating, reviews: parseInt(reviews)||0, availability, bullets});
})()
""")

info = json.loads(detail)
print(info)
```

### Availability string patterns

```python
avail = info["availability"]
if "In Stock" in avail:
    status = "in_stock"
elif re.search(r"Only \d+ left", avail):
    n = re.search(r"(\d+)", avail).group(1)
    status = f"low_stock_{n}"
elif "Usually ships" in avail:
    status = "ships_delayed"
elif "Unavailable" in avail or "Currently unavailable" in avail:
    status = "out_of_stock"
else:
    status = "unknown"
```

---

## Workflow 3: Price checking by ASIN (fast, no browser)

For a single known ASIN, `http_get` is usually sufficient and avoids CAPTCHA risk:

```python
import re

asin = "B08N5WRWNW"
html = http_get(
    f"https://www.amazon.com/dp/{asin}",
    headers={"Accept-Language": "en-US,en;q=0.9"}
)

# Extract title
title_m = re.search(r'id="productTitle"[^>]*>\s*([^<]+)', html)
title = title_m.group(1).strip() if title_m else "N/A"

# Extract price (offscreen span is most reliable in raw HTML)
price_m = re.search(r'class="a-offscreen">([^<]+)</span>', html)
price = price_m.group(1).strip() if price_m else "N/A"

print(f"{asin}: {title} — {price}")
```

If `http_get` returns a CAPTCHA page (check `"captcha" in html.lower()`), fall back to the browser path (goto + wait_for_load + js extraction above).

```python
if "captcha" in html.lower() or "Type the characters" in html:
    # Fall back to browser
    goto(f"https://www.amazon.com/dp/{asin}")
    wait_for_load()
    wait(2)
    # ... use js() extraction from Workflow 2
```

---

## Workflow 4: Best Sellers extraction

```python
import json

category_path = "electronics"   # or "computers", "books", etc.
goto(f"https://www.amazon.com/Best-Sellers/zgbs/{category_path}/")
wait_for_load()
wait(2)

bestsellers = js("""
(function() {
  var items = document.querySelectorAll(
    '#zg-ordered-list li, .zg-item-immersion'
  );
  var out = [];
  for (var i = 0; i < Math.min(items.length, 10); i++) {
    var el = items[i];
    var rank = i + 1;
    var titleEl = el.querySelector('.zg-bdg-text, ._cDEzb_p13n-sc-css-line-clamp-3_g3dy1, .p13n-sc-truncated');
    var title = titleEl ? titleEl.innerText.trim() : '';
    var priceEl = el.querySelector('.p13n-sc-price, ._cDEzb_p13n-sc-price_3mJ9Z');
    var price = priceEl ? priceEl.innerText.trim() : '';
    var ratingEl = el.querySelector('.a-icon-star-small span, .a-icon-star span');
    var rating = ratingEl ? ratingEl.innerText.trim() : '';
    var linkEl = el.querySelector('a.a-link-normal[href*="/dp/"]');
    var href = linkEl ? linkEl.href : '';
    var asinM = href.match(/\/dp\/([A-Z0-9]{10})/);
    var asin = asinM ? asinM[1] : '';
    if (title) out.push({rank, asin, title, price, rating});
  }
  return JSON.stringify(out);
})()
""")

items = json.loads(bestsellers)
for item in items:
    print(item)
```

Best Sellers pages paginate in two columns — if `items` is empty, the page may use a different layout. Take a screenshot to confirm:

```python
screenshot()  # inspect layout before tuning selectors
```

---

## Workflow 5: Multi-page search (pagination)

Amazon search results have a "Next" button. Extract the URL from it rather than incrementing a page counter (page numbers can be non-linear after filter application):

```python
import json

all_results = []
goto("https://www.amazon.com/s?k=standing+desk&rh=p_36%3A1000-30000")
wait_for_load()
wait(2)

for page_num in range(1, 6):  # up to 5 pages
    # extract current page results (same JS as Workflow 1)
    batch = json.loads(js("""
    (function() {
      var cards = document.querySelectorAll('[data-component-type="s-search-result"]');
      var out = [];
      cards.forEach(function(c) {
        var asin = c.getAttribute('data-asin') || '';
        var titleEl = c.querySelector('h2 a span');
        var title = titleEl ? titleEl.innerText.trim() : '';
        var priceEl = c.querySelector('.a-price .a-offscreen');
        var price = priceEl ? priceEl.innerText.trim() : '';
        if (asin && title) out.push({asin, title, price});
      });
      return JSON.stringify(out);
    })()
    """))
    all_results.extend(batch)

    # get next page URL
    next_url = js("""
      var n = document.querySelector('.s-pagination-next:not(.s-pagination-disabled)');
      n ? n.href : null
    """)
    if not next_url:
        break

    goto(next_url)
    wait_for_load()
    wait(2)  # respect rate limit between pages

print(f"Total results collected: {len(all_results)}")
```

---

## Workflow 6: Add to cart (requires logged-in session)

This only works when Chrome already has an active Amazon session.

```python
asin = "B09XLNBM3B"
goto(f"https://www.amazon.com/dp/{asin}")
wait_for_load()
wait(2)

# Verify not a CAPTCHA or login redirect
url = page_info()["url"]
if "signin" in url or "ap/signin" in url:
    raise RuntimeError("Not logged in — open Amazon and sign in first")

# Take screenshot to find Add to Cart button location
screenshot()

# Click "Add to Cart" (coordinate from screenshot — typically center of page, ~y=400-600)
# Use JS to click it reliably if coordinates are uncertain
added = js("""
(function() {
  var btn = (
    document.getElementById('add-to-cart-button') ||
    document.querySelector('input[name="submit.add-to-cart"]') ||
    document.querySelector('input[id*="add-to-cart"]')
  );
  if (!btn) return 'not_found';
  btn.click();
  return 'clicked';
})()
""")

wait(3)
wait_for_load()

# Confirm: cart overlay or redirect to cart page
url_after = page_info()["url"]
confirmation = js("document.querySelector('#NATC_SMART_WAGON_CONF_MSG_SUCCESS, #sw-atc-confirmation') ? 'success' : 'check_page'")
print(f"Result: {added}, {confirmation}, URL: {url_after}")
```

---

## ASIN extraction patterns

ASINs are always 10 characters, uppercase letters and digits. Three reliable sources:

```python
import re

# 1. From the current page URL
asin_from_url = re.search(r'/dp/([A-Z0-9]{10})', page_info()["url"])
asin = asin_from_url.group(1) if asin_from_url else None

# 2. From a card's data-asin attribute
asin = js("document.querySelector('[data-asin]').getAttribute('data-asin')")

# 3. From raw HTML or href text
asins_in_html = re.findall(r'/dp/([A-Z0-9]{10})', some_html_string)
```

---

## Price format handling

Amazon serves prices in several formats — handle all of them:

```python
import re

def parse_price(raw: str) -> float | None:
    """Normalize any Amazon price string to a float, or None if missing."""
    if not raw:
        return None
    raw = raw.strip()
    # "from $12.99" → strip "from"
    raw = re.sub(r'(?i)^from\s+', '', raw)
    # "$1,299.00" or "$12.99" or "$12" (no cents)
    m = re.search(r'\$?([\d,]+)\.?(\d{0,2})', raw.replace(',', ''))
    if not m:
        return None
    dollars = m.group(1).replace(',', '')
    cents = m.group(2).ljust(2, '0') if m.group(2) else '00'
    try:
        return float(f"{dollars}.{cents}")
    except ValueError:
        return None

# Price can also come split across two DOM elements:
#   .a-price-whole = "12." (trailing dot)
#   .a-price-fraction = "99"
# In that case: float(whole.rstrip('.') + '.' + frac)
```

---

## Gotchas

- **CAPTCHA wall** — Amazon detects rapid automated requests. Symptoms: page title is "Robot Check" or "CAPTCHA", body contains "Type the characters". Always `wait(2)` between page loads. If hit, take a `screenshot()` — sometimes the CAPTCHA is solvable visually. For bulk scraping, use `http_get` in a `ThreadPoolExecutor` with at most 3-5 concurrent requests.

  ```python
  def is_captcha(html: str) -> bool:
      return "Type the characters" in html or "captcha" in html.lower() or "Robot Check" in html
  ```

- **Login-gated prices** — Some prices only show after sign-in (Prime pricing, business pricing, Warehouse Deals). If `price == ""` but the product exists, you are seeing the logged-out view. The `http_get` path is more likely to hit this than the browser path.

- **Split dollar/cents DOM structure** — The `.a-price` container splits into `.a-price-whole` (e.g. `"299."`) and `.a-price-fraction` (e.g. `"99"`). The `.a-offscreen` child has the full combined string and is the most reliable single target.

- **"Sponsored" results at the top** — Filter with `:not([data-component-type="sp-sponsored-result"])`. Sponsored cards still have `data-asin` set, so ASIN-only extraction picks them up unless filtered.

- **Third-party seller prices absent until cart** — For listings fulfilled by third-party sellers, the price element may be empty. The page shows "See price in cart" — this is intentional, not a scraping failure.

- **Review count formatting** — Amazon renders `"1,234 ratings"`. Strip commas before `int()`:
  ```python
  count = int(review_text.replace(",", "").split()[0])
  ```

- **Best Sellers layout shift** — The Best Sellers page has redesigned its layout multiple times. If the `#zg-ordered-list` selector misses, fall back to `.zg-item-immersion` or take a screenshot and re-inspect. The ASIN is always extractable from `href` links containing `/dp/`.

- **TLD differences** — Prices, availability text, and some DOM IDs differ by country. The `.a-price .a-offscreen` pattern is reliable cross-TLD. For `.co.uk`, `.de`, `.fr`, `.co.jp` — replace the base URL; product data structure is the same. Currency symbols change (`£`, `€`, `¥`).

- **Dynamic price loading** — Some product pages load price via XHR after DOMContentLoaded. `wait_for_load()` alone is not enough — add `wait(2)` after it before running JS extraction.

- **Page variants** — Amazon A/B tests layouts constantly. If extraction returns empty strings, `screenshot()` immediately to see actual page state. Selectors documented here reflect the most common variant as of early 2026.

- **Robot detection escalation** — If requests succeed but return increasingly sparse data (no prices, empty titles), Amazon may be in a soft-block state serving degraded content. Take a screenshot, check the actual page, and add more `wait()` between requests.

- **`data-component-type` is not always present** — On some search result pages (especially category browse pages reached via `/s?i=...`), `data-component-type` is absent. Fall back to `[data-asin]` directly and tolerate empty fields.
