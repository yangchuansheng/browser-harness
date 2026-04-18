# GitHub — Scraping public data

`https://github.com` and `https://api.github.com` — public repo metadata, user profiles, trending pages, releases, and README content.

## Do this first

**Use the REST API with `http_get`, not the browser.** For anything that has an API endpoint (repo metadata, user profiles, releases, tags, README), a single `http_get` call replaces `goto + wait_for_load + screenshot + js`. The browser is only needed for pages that render client-side without a JSON equivalent (trending page, search results with JS filtering).

```python
import json, os

def gh_get(path, token=None):
    headers = {"Accept": "application/vnd.github+json"}
    tok = token or os.environ.get("GITHUB_TOKEN")
    if tok:
        headers["Authorization"] = f"token {tok}"
    return json.loads(http_get(f"https://api.github.com{path}", headers=headers))

# Repo metadata — stars, forks, description, topics, language, license
repo = gh_get("/repos/browser-use/browser-use")
print(repo["stargazers_count"], repo["forks_count"], repo["description"])
```

Unauthenticated: **60 requests/hour** per IP. With a token: **5000/hour**. Always check `X-RateLimit-Remaining` before looping.

---

## Common workflows

### 1. Trending repositories (browser required)

The trending page (`https://github.com/trending`) is rendered client-side — `http_get` returns a skeleton with no repo data. Use the browser.

```python
goto("https://github.com/trending")
wait_for_load()

# Optional: filter by language and period via URL params
# goto("https://github.com/trending/python?since=weekly")
# goto("https://github.com/trending/typescript?since=daily")

repos = js("""
(function() {
  var rows = document.querySelectorAll('article.Box-row');
  return Array.from(rows).map(function(row) {
    var nameEl = row.querySelector('h2 a');
    var descEl = row.querySelector('p');
    var starsEl = row.querySelector('a[href$="/stargazers"]');
    var forksEl = row.querySelector('a[href$="/forks"]');
    var langEl  = row.querySelector('[itemprop="programmingLanguage"]');
    var todayEl = row.querySelector('.float-sm-right');
    return {
      full_name:   nameEl ? nameEl.getAttribute('href').slice(1) : null,
      url:         nameEl ? 'https://github.com' + nameEl.getAttribute('href') : null,
      description: descEl ? descEl.textContent.trim() : null,
      stars:       starsEl ? starsEl.textContent.trim() : null,
      forks:       forksEl ? forksEl.textContent.trim() : null,
      language:    langEl  ? langEl.textContent.trim() : null,
      stars_today: todayEl ? todayEl.textContent.trim() : null,
    };
  });
})()
""")
print(repos[:10])
```

**Note:** `stars` from the DOM will be "1.2k" not `1234`. Use the API for exact counts if you need them:

```python
from concurrent.futures import ThreadPoolExecutor

def enrich(repo):
    data = gh_get(f"/repos/{repo['full_name']}")
    repo["stars_exact"] = data["stargazers_count"]
    repo["forks_exact"] = data["forks_count"]
    return repo

with ThreadPoolExecutor(max_workers=8) as ex:
    enriched = list(ex.map(enrich, repos))
```

---

### 2. User profile — followers, bio, public repos

```python
# API (preferred)
user = gh_get("/users/Archish27")
print(user["followers"])       # int, exact
print(user["following"])
print(user["public_repos"])
print(user["bio"])
print(user["avatar_url"])
print(user["blog"])            # personal site if set
print(user["company"])
print(user["location"])
```

Fields always present: `login`, `id`, `avatar_url`, `html_url`, `type`, `public_repos`, `public_gists`, `followers`, `following`, `created_at`, `updated_at`.

---

### 3. Repository metadata — stars, forks, topics, language

```python
repo = gh_get("/repos/n4ze3m/page-assist")

print({
    "stars":       repo["stargazers_count"],
    "forks":       repo["forks_count"],
    "watchers":    repo["watchers_count"],
    "open_issues": repo["open_issues_count"],
    "language":    repo["language"],          # primary language
    "topics":      repo["topics"],            # list[str]
    "license":     repo["license"]["name"] if repo["license"] else None,
    "description": repo["description"],
    "created_at":  repo["created_at"],
    "pushed_at":   repo["pushed_at"],         # last commit time
    "default_branch": repo["default_branch"],
    "archived":    repo["archived"],
})
```

---

### 4. README content

```python
import base64

readme = gh_get("/repos/browser-use/browser-use/readme")
# content is base64-encoded
text = base64.b64decode(readme["content"]).decode("utf-8")
print(text[:2000])
```

Alternatively, raw markdown (no auth needed, no rate limit from the API quota):

```python
raw = http_get("https://raw.githubusercontent.com/browser-use/browser-use/main/README.md")
print(raw[:2000])
```

`raw.githubusercontent.com` is a plain CDN — not rate-limited like the API. Prefer it for README-only tasks.

---

### 5. Latest releases and tags

```python
# Latest release (published, non-prerelease)
release = gh_get("/repos/owner/repo/releases/latest")
print(release["tag_name"])      # e.g. "v1.2.3"
print(release["name"])          # release title
print(release["body"])          # release notes (markdown)
print(release["published_at"])
print(release["html_url"])

# Assets (download URLs)
for asset in release["assets"]:
    print(asset["name"], asset["browser_download_url"])

# All releases (paginated)
releases = gh_get("/repos/owner/repo/releases?per_page=10")
for r in releases:
    print(r["tag_name"], r["published_at"], r["prerelease"])

# Tags (lighter — no release notes)
tags = gh_get("/repos/owner/repo/tags?per_page=10")
for t in tags:
    print(t["name"], t["commit"]["sha"])
```

---

### 6. Parallel multi-repo fetch

```python
import json, os
from concurrent.futures import ThreadPoolExecutor

repos_to_check = [
    "browser-use/browser-use",
    "n4ze3m/page-assist",
    "microsoft/vscode",
]

def fetch_repo(full_name):
    try:
        return gh_get(f"/repos/{full_name}")
    except Exception as e:
        return {"full_name": full_name, "error": str(e)}

with ThreadPoolExecutor(max_workers=10) as ex:
    results = list(ex.map(fetch_repo, repos_to_check))

for r in results:
    if "error" not in r:
        print(r["full_name"], r["stargazers_count"])
```

---

### 7. Most starred repos for a language (Search API)

```python
# Top Python repos by stars from the past week
import urllib.parse

q = urllib.parse.quote("language:python created:>2026-04-11")
results = gh_get(f"/search/repositories?q={q}&sort=stars&order=desc&per_page=10")
for item in results["items"]:
    print(item["full_name"], item["stargazers_count"])
```

Search API counts against rate limits more aggressively — **30 requests/min** unauthenticated, **30/min** authenticated (same for search). Add a `GITHUB_TOKEN` to avoid 422/403 on busy loops.

---

## When to use the browser vs pure HTTP

| Task | Method |
|---|---|
| Repo metadata (stars, forks, desc, topics) | `http_get` → API |
| User profile (followers, bio, avatar) | `http_get` → API |
| README text | `http_get` → raw CDN |
| Latest release / tags | `http_get` → API |
| Trending page (all languages) | Browser (`goto` + `js`) |
| Trending page filtered by language | Browser (`goto` + `js`) |
| Search results with JS-driven filters | Browser |
| Paginated issue / PR lists | `http_get` → API (`?page=N&per_page=100`) |
| File contents in a repo | `http_get` → `raw.githubusercontent.com` |
| Private repos | Browser (authenticated session) OR API with token |

---

## Gotchas

- **Unauthenticated rate limit is 60 req/hr total per IP** — not per endpoint. A loop over 70 repos will hit it. Set `GITHUB_TOKEN` in `.env` or the environment:

  ```python
  import os
  # Set once; gh_get() picks it up automatically via os.environ.get("GITHUB_TOKEN")
  # Don't hardcode the token in scripts
  ```

- **Trending page needs the browser, not `http_get`.** `http_get("https://github.com/trending")` returns the HTML skeleton; the repo list is injected by a React bundle after `DOMContentLoaded`. Always use `goto` + `wait_for_load` + `js(...)`.

- **Star counts in the trending DOM are formatted strings ("1.2k", "23.4k"), not integers.** Use the API if you need exact numbers. Quick parser if you only have DOM values:

  ```python
  def parse_stars(s):
      s = s.strip().replace(",", "")
      if s.endswith("k"):
          return int(float(s[:-1]) * 1000)
      return int(s)
  ```

- **The trending page `article.Box-row` selector is stable as of 2026-04** but GitHub redesigns without notice. If `js(...)` returns an empty list, `screenshot()` to see the current DOM structure and update the selector.

- **`/repos/{owner}/{repo}` 404s if the repo is private or deleted** — wrap in try/except and check `response["message"]`.

- **Search API has a separate lower rate limit (30/min) and requires queries to have at least one qualifier.** `q=stars:>1000` works; `q=` alone returns a 422.

- **`raw.githubusercontent.com` is NOT rate-limited by the GitHub API quota** — use it freely for file content, README, and configs. It's a CDN, not the API gateway.

- **Pagination:** API endpoints default to `per_page=30`, max `per_page=100`. For full lists:

  ```python
  def paginate(path):
      results, page = [], 1
      while True:
          sep = "&" if "?" in path else "?"
          batch = gh_get(f"{path}{sep}per_page=100&page={page}")
          if not batch:
              break
          results.extend(batch)
          if len(batch) < 100:
              break
          page += 1
      return results

  all_releases = paginate("/repos/owner/repo/releases")
  ```

- **GraphQL API** (not shown above) at `https://api.github.com/graphql` supports batching many repos in one request — worth it if you're pulling data for 50+ repos and want to stay under rate limits. Requires a token with appropriate scopes.

- **Lazy loading on long repo lists in the browser:** GitHub infinite-scrolls contributor lists and file trees. If `js(...)` returns fewer items than expected, scroll first:

  ```python
  scroll(760, 400, dy=3000)
  wait(1.5)
  # then re-run the js() extraction
  ```
