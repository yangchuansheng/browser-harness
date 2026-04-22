# LinkedIn — public profiles, Pages, and admin workflows

Starter guide based on LinkedIn Help pages. Treat LinkedIn as a browser-first
domain: public URLs exist, but visibility, gating, and admin surfaces vary by
account and role.

## Do this first

- Decide whether you are working with a public member profile, a public company
  Page, or a Page admin surface.
- Prefer direct known URLs over global search. Search, feed, and recommendation
  views are highly personalized.
- Use a real signed-in browser for repeated navigation or any admin workflow.
  Public profile visibility depends on member settings, and Page admin access is
  tied to a real member account.

## Stable URL patterns

### Public member profiles

LinkedIn's public profile help documents the canonical public profile shape as:

```text
https://www.linkedin.com/in/<public-profile-slug>/
```

Language-specific public profile variants append the language code:

```text
https://www.linkedin.com/in/<public-profile-slug>/<lang>/
```

Examples from LinkedIn Help use `/in/username` and `/in/username/es`.

### Company Pages

Company Pages are organization surfaces. In practice, the durable public entry
point is the company slug under `/company/`:

```text
https://www.linkedin.com/company/<company-slug>/
```

Useful follow-on surfaces usually hang off the Page path:

```text
https://www.linkedin.com/company/<company-slug>/about/
https://www.linkedin.com/company/<company-slug>/jobs/
https://www.linkedin.com/company/<company-slug>/people/
```

Do not assume every tab is visible for every viewer. LinkedIn changes which
tabs are shown based on account state, region, and product enablement.

### Page admin workflows

LinkedIn Help is explicit that Page admins do **not** use a separate Page
login. Admin access is attached to an individual member profile. The supported
entry path is:

1. Sign in to the member profile that has admin access.
2. Use the **My pages** pane on the homepage.
3. Open the desired Page from there.

That means automation should start from the signed-in homepage or a known Page
URL in a browser session that already has the correct member authenticated.

## Page model notes

- LinkedIn distinguishes **Pages** and **Showcase Pages**. Showcase Pages are
  sub-brands or initiatives and do not replace the main company Page.
- Public URLs must be unique. LinkedIn Help calls this out for targeted Page
  surfaces, and the same operational assumption is safe for any vanity URL:
  slugs are user-facing and can change.
- Public member profiles are optional. If a member disables public visibility,
  the URL may redirect to a sign-in wall or a reduced page.
- Admin access is role-based. Super admins, content admins, analysts, and paid
  media admins do not all see the same surface.

## Good starting workflows

### Inspect a public member profile

- Start from a known `/in/<slug>/` URL.
- If the page is not visible without login, stop treating it as a public path
  and move to a signed-in browser workflow.
- Prefer the canonical public profile URL over search results, which are noisy
  and personalized.

### Inspect a company Page

- Start from `/company/<slug>/`.
- Move to `/about/` for company metadata and `/jobs/` for role listings rather
  than depending on whatever tab the landing page currently highlights.
- If you need to manage the Page, switch to the signed-in admin workflow rather
  than trying to force actions from the public member view.

### Access a Page admin surface

- Use the signed-in member profile that already has Page permissions.
- Open the Page from **My pages** on the homepage.
- Expect different left-nav items depending on role and enabled products.

## Traps

- Global search is not a durable entrypoint. Result ranking and even visible
  entity types vary by account and geography.
- Vanity slugs are human-facing, not immutable IDs. Bookmark with care.
- Public profile and Page visibility are not universal. Some URLs work only
  when signed in or when a viewer has the right relationship to the Page.
- Admin workflows inherit the member account, not a separate Page session.
  Wrong account means wrong Page surface.
