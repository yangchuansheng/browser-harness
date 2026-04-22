# Salesforce — Lightning Experience starter

Starter guide based on Salesforce developer documentation. Treat Salesforce as
an org-specific, browser-first domain: hosts, enabled apps, and visible objects
vary by org, and Salesforce does not officially support UI URLs as a public
interface contract.

## Do this first

- Work from a browser session that is already authenticated into the target org.
- Prefer relative Lightning paths under the active org instead of hardcoding a
  hostname.
- Use direct URLs only as entrypoints. Salesforce's own docs say UI URLs can
  change, so stable automation should rely on visible navigation after landing.
- If you are automating inside a known app, start from the app and then move to
  object or record pages. App context changes available tabs and list views.

## Stable Lightning URL patterns

Salesforce's Lightning URL update docs give these durable entrypoint shapes:

### Object home

```text
/lightning/o/<ObjectApiName>/home
```

Example from Salesforce docs:

```text
/lightning/o/Account/home
```

### Record view

```text
/lightning/r/<ObjectApiName>/<recordId>/view
```

Example from Salesforce docs:

```text
/lightning/r/Account/<recordId>/view
```

### Common adjacent routes

These are standard Lightning variations commonly exposed from the same record
or object surfaces:

```text
/lightning/r/<ObjectApiName>/<recordId>/edit
/lightning/o/<ObjectApiName>/list?filterName=<listViewIdOrName>
/lightning/setup/<SetupPageName>/home
```

The old `one/one.app#/...` shapes still appear in docs and old links, but
Lightning rewrites them to `/lightning/...` when possible.

## Route and state notes

- Salesforce explicitly warns that UI URLs are not a supported API surface.
- Query parameters on old `one.app` URLs are rewritten when entered in a
  browser.
- Lightning is a single-page app. A URL change does not guarantee a full page
  reload, and a full page reload is not required for visible state changes.
- If you need reliable deep-link generation in Salesforce code, the official
  path is `PageReference` + `lightning/navigation`, not handcrafted URLs.

## Good starting workflows

### Open a known object

If you know the object API name:

```text
/lightning/o/Account/home
/lightning/o/Opportunity/home
/lightning/o/Case/home
```

Use the object home as the safest first landing page before choosing a list
view or opening a record.

### Open a known record

If you know both the object API name and record ID:

```text
/lightning/r/Account/001.../view
```

Do not guess the object API name from labels in the UI. Use the actual API name
when building the route.

### Enter Setup

Setup lives under `/lightning/setup/...`. Because setup pages differ by org and
enabled products, use Setup search or visible nav once you are inside Setup
instead of hardcoding deep admin URLs unless you already know the exact page.

## Traps

- My Domain hosts vary by org, sandbox, and region. The same path can live on
  different hosts in different environments.
- UI URLs are not contract-stable. Land on them, then use visible app state to
  continue.
- Object API names matter. `Account` and `Accounts` are not interchangeable.
- Old `one.app` links may still work, but Lightning rewrites them; do not build
  new automation around the legacy hash format.
- URL-only changes inside Lightning do not always rerender the visible page.
  Verify page chrome and object context after navigation.
