# Profile Sync

Make a remote Browser Use browser start already logged in by syncing cookies
from a local Chrome profile into a cloud profile first.

This is control-plane work, not a guest-boundary workflow.

## Rust-Native Admin Surface

Use the top-level Rust CLI:

- `browser-harness list-cloud-profiles`
- `browser-harness resolve-profile-name <name>`
- `browser-harness list-local-profiles`
- `browser-harness sync-local-profile`
- `browser-harness create-browser`
- `browser-harness stop-browser <browser-id>`

## One-Time Install

```bash
curl -fsSL https://browser-use.com/profile.sh | sh
```

That installs `profile-use`, which `browser-harness sync-local-profile` shells
out to under the hood.

## Listing Cloud And Local Profiles

```bash
browser-harness list-cloud-profiles
browser-harness list-local-profiles
```

Use this first instead of guessing which profile to reuse or sync.

## Sync A Local Profile

`sync-local-profile` reads JSON on stdin:

```bash
browser-harness sync-local-profile <<'JSON'
{"profileName":"Default","browser":"Google Chrome","cloudProfileId":"<optional-existing-id>","includeDomains":["stripe.com"],"excludeDomains":[]}
JSON
```

Output shape:

```json
{"cloudProfileId":"...","stdout":"...","stderr":"..."}
```

Use `cloudProfileId` when you want to refresh an existing cloud profile instead
of creating duplicates.

## Start A Remote Browser With That Profile

Create the remote browser with the cloud profile id in the Browser Use browser
payload you send to `create-browser`.

If you only have a profile name, resolve it first:

```bash
browser-harness resolve-profile-name my-work-profile
```

## Chat-Driven Rule

Do not choose or sync an auth-bearing profile unilaterally.

Preferred operator flow:

1. list existing cloud profiles
2. ask whether to reuse, resync, or start clean
3. if syncing local, show the detected local profiles first
4. scope domains deliberately when that is safer than syncing everything

## What Actually Syncs

Cookies only.

It does not sync:

- localStorage
- IndexedDB
- extensions

So this is useful for session-cookie sites, but not for sites that keep auth
only in local storage.

## Traps

- Browser Use proxy settings can still break certain destinations with
  `ERR_TUNNEL_CONNECTION_FAILED`
- profile names are not unique; prefer explicit cloud profile ids when updating
- dedicated work profiles are safer than personal profiles for repeated testing
