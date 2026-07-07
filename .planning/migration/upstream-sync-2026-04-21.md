# Upstream Sync Audit — 2026-04-21+

## Scope

- Upstream repository: `https://github.com/browser-use/browser-harness`
- Baseline commit before requested date: `2d23211d346c7a12bdb2ce03e49b2d955f4769b2`
- Upstream target commit: `4d75f115c039bf769d614fbd8d996a961e143567`
- Commit range: `2d23211d346c7a12bdb2ce03e49b2d955f4769b2..4d75f115c039bf769d614fbd8d996a961e143567`
- Count: 302 commits
- User intent: replicate all upstream updates since Apr 21, 2026 into this Rust fork while preserving the Rust architecture.

## Migrated Runtime Behavior

- Added expanded local browser profile discovery for Chrome Canary, Comet, Arc,
  Dia, Brave, Edge channels, Windows Chrome SxS, and Flatpak profile paths.
- Added `BU_CDP_URL` DevTools HTTP endpoint support alongside `BU_CDP_WS`.
- Added `/json/version` websocket resolution and `DevToolsActivePort` fallback
  for newer Chrome builds returning 404.
- Split runtime and temp paths with `BH_RUNTIME_DIR` for socket/pid files and
  `BH_TMP_DIR` for logs/screenshots.
- Added `BU_NAME` validation to prevent path traversal in runtime file names.
- Added daemon `ping` and `connection_status` metadata.
- Updated controlled-tab marker to 🐴 and fixed marker removal.
- Preserved target attachment status for `current_tab` and `set_session` flows.
- Added remote-specific CDP handshake messaging for cloud websocket failures.

## Migrated Helper/API Surface

- Added `wait_for_element` / `wait-for-element` for SPA late-render waits.
- Added `fill_input` / `fill-input` for framework-managed inputs.
- Added `wait_for_network_idle` / `wait-for-network-idle` for XHR/fetch settle waits.
- Added screenshot `max_dim` support with Rust PNG resize behavior.
- Exposed the new operations through `bhrun`, `browser-harness`, `bh-wasm-host`,
  and `bh_guest_sdk`.
- Added remote-browser upload staging parity from upstream commits `f226972`/`e87f8b7`: local files are staged into `/tmp/browser-harness-uploads` inside the browser host before `DOM.setFileInputFiles`; WASM guests can provide base64 upload payloads through `upload_file_data` / `upload_remote_files`.

## Migrated Knowledge and Docs

- Imported upstream domain-skill corpus into `domains/` with upstream
  `scraping.md` mapped to this fork's `skill.md` convention.
- Added the QuickBooks Online (`qbo`) report-export domain skill from upstream PR #314 as `domains/qbo/report-export.md`.
- Imported upstream issue templates and `VOUCHED.td`.
- Imported `docs/snap-linux-headless.md` and `docs/allow-remote-debugging.png`.
- Updated `SKILL.md`, `install.md`, `README.md`, `domains/README.md`, and
  interaction skills for upstream connection and helper guidance.
- Linked the upstream Browser Use Box deployment demo in `README.md`.
- Updated upload and WASM guest docs for remote-browser staging behavior.

## Adapted Instead of Copied

- Python runtime files (`src/browser_harness/*.py`) were not copied verbatim;
  equivalent behavior was ported to Rust crates and binaries.
- Upstream GitHub workflows were not copied so the Rust CI/workspace shape is
  not overwritten by Python packaging assumptions.
- Existing Rust architecture, WASM guest model, and Rust-specific docs were kept.

## Verification Evidence

- `cargo fmt --manifest-path rust/Cargo.toml --all -- --check` passed.
- `cargo check --manifest-path rust/Cargo.toml --workspace` passed.
- `env -u CFLAGS -u CC cargo test --manifest-path rust/Cargo.toml --workspace` passed.
- `env -u CFLAGS -u CC cargo run --quiet --manifest-path rust/Cargo.toml --bin bhrun -- summary` exposed the new helper operations.
- `env -u CFLAGS -u CC cargo run --quiet --manifest-path rust/Cargo.toml --bin browser-harness -- --help` exposed the new runner commands through the facade.
- `git diff --check` passed.
- Secret/local-path scans found no API keys, pinned local websocket, or local home path leaks in tracked/unignored files.

## Re-Audit — 2026-05-14

- Re-fetched `upstream/main`; target remains `2f22ed6709748edc5eab733eae099802640a78e2`.
- Recounted commit range `2d23211d346c7a12bdb2ce03e49b2d955f4769b2..upstream/main`: 239 commits.
- Cross-checked upstream domain-skill entries from both `agent-workspace/domain-skills/` and legacy `domain-skills/` paths against this fork's `domains/` mapping.
- Initial re-audit found two missing legacy Amazon domain-skill files from upstream commit `17e88b4`: `domain-skills/amazon/cart.md` and `domain-skills/amazon/orders.md`.
- Fixed by adding Rust-layout equivalents at `domains/amazon/cart.md` and `domains/amazon/orders.md`; helper examples use text fences and path references follow the local `domains/` convention.
- Post-fix domain mapping result: 109 upstream domain-skill entries / 109 local mapped files present.

## Re-Audit Verification Evidence

- `git fetch upstream main` confirmed target `2f22ed6709748edc5eab733eae099802640a78e2`.
- Domain mapping script reported `upstream domain file entries 109` and `missing mapped files 0`.
- Re-ran Rust formatting, check, tests, CLI smoke, diff whitespace check, and secret/local-path scans after the Amazon fix. The repository `scripts/scan_sensitive.sh` requires Bash 4 `mapfile`; macOS `/bin/bash` is 3.2 in this worktree, so an equivalent Python/rg scan was used for the final secret/local-path pass.


## Daily Upstream Sync — 2026-05-15

- Fetched `origin/main` and `upstream/main`; local `main` started clean and equal to `origin/main`.
- Previous target: `2f22ed6709748edc5eab733eae099802640a78e2`; new upstream target: `caebe67fc780482bc9c57e88872f62cdb5a9b42d`.
- New upstream range `2f22ed6709748edc5eab733eae099802640a78e2..upstream/main`: 4 commits.
- Upstream changes analyzed:
  - `f226972` / PR `e87f8b7`: remote-browser file upload staging in Python helpers plus unit tests.
  - `bdd550b` / PR `caebe67`: Browser Use Box deployment-demo README link.
- Rust migration decisions:
  - Ported remote upload staging into `bh-daemon` instead of copying Python runtime files. The daemon now detects remote CDP sessions from `BU_BROWSER_ID` or non-loopback `BU_CDP_WS` / `BU_CDP_URL`, stages local files through browser downloads, then resolves a fresh file input before `DOM.setFileInputFiles`.
  - Added `remote_files` payload support to the typed `UploadFileRequest` and guest SDK helpers for in-memory/base64 upload data.
  - Preserved local-browser behavior: local uploads still pass local paths directly unless remote staging is enabled.
  - Updated `README.md`, `interaction-skills/uploads.md`, and `docs/wasm-guests.md` for the new behavior and upstream Browser Use Box link.

## Daily Sync Verification Evidence — 2026-05-15

- `cargo fmt --manifest-path rust/Cargo.toml --all -- --check` passed.
- `cargo check --manifest-path rust/Cargo.toml --workspace` passed.
- `env -u CFLAGS -u CC cargo test --manifest-path rust/Cargo.toml --workspace` passed.
- `env -u CFLAGS -u CC cargo run --quiet --manifest-path rust/Cargo.toml --bin bhrun -- summary` passed and reports `upload_file=live`.
- `env -u CFLAGS -u CC cargo run --quiet --manifest-path rust/Cargo.toml --bin browser-harness -- --help` passed and exposes `upload-file` through the facade.
- `git diff --check` passed.
- `./scripts/scan_sensitive.sh` still fails on macOS Bash 3.2 because it uses Bash 4 `mapfile`; an equivalent Python scan over tracked/unignored files passed with no obvious secrets or local path leaks.

## Daily Upstream Sync — 2026-05-17

- Fetched `upstream/main` and reviewed upstream ancestry after the prior sync target.
- Previous target: `caebe67fc780482bc9c57e88872f62cdb5a9b42d`; new upstream target: `9e47d2b7775404094e977d3297d8a41e09f73a81`.
- New upstream range `caebe67fc780482bc9c57e88872f62cdb5a9b42d..9e47d2b7775404094e977d3297d8a41e09f73a81`: 4 non-merge commits, plus merge commits on `upstream/main`.
- Upstream changes analyzed:
  - `f2dca2b`: added `llms.txt` with Browser Use Box discovery link.
  - `87fe826`: reverted the Browser Use Box deployment-demo README link.
  - `93ce332`: reverted `llms.txt`.
  - `1599ba1`: reverted remote-browser upload staging from Python helpers and removed related tests.
- Net upstream effect: `llms.txt` add/revert cancels out; the durable changes are two effective reverts.
- Rust migration decisions:
  - Removed the Rust port of remote-browser upload staging from `bh-daemon`; `upload_file` again passes the caller-supplied file paths directly to `DOM.setFileInputFiles`.
  - Removed `remote_files` from `UploadFileRequest`, `bhrun` request forwarding, `bh-wasm-host`, and `bh_guest_sdk`.
  - Removed guest SDK in-memory/base64 upload helpers (`upload_file_data` and `upload_remote_files`) because upstream reverted that behavior.
  - Removed daemon remote-staging detection from `bhd` and the `sha2` dependency that only supported staged upload filenames.
  - Removed the Browser Use Box deployment-demo link from `README.md`; no `llms.txt` file is present after the upstream add/revert pair.
  - Updated `interaction-skills/uploads.md` and `docs/wasm-guests.md` so upload guidance matches simple path passing again.

## Daily Sync Verification Evidence — 2026-05-17

- `cargo fmt --manifest-path rust/Cargo.toml --all -- --check` passed.
- `cargo check --manifest-path rust/Cargo.toml --workspace` passed.
- `env -u CFLAGS -u CC cargo test --manifest-path rust/Cargo.toml --workspace` passed.
- `git diff --check` passed.
- `python3` tracked-file secret scan plus `rg` checks passed with no obvious secrets, local home paths, `llms.txt`, Browser Use Box demo link, `remote_files`, or remote upload staging remnants in active code/docs.

## Daily Upstream Sync — 2026-05-20

- Started from clean local `main` at `3f5002175246755fba081379b71921fd026fb8ae`, equal to `origin/main`; `upstream/main` was `ea7d1710ba8621c658d6d61fe46bcf77746e83e4`.
- Previous target: `9e47d2b7775404094e977d3297d8a41e09f73a81`; new upstream target: `ea7d1710ba8621c658d6d61fe46bcf77746e83e4`.
- New upstream range `9e47d2b7775404094e977d3297d8a41e09f73a81..ea7d1710ba8621c658d6d61fe46bcf77746e83e4`: 3 non-merge commits plus 2 merge commits.
- Upstream changes analyzed:
  - `e0e7f0b`: added Python `close_tab(target=None)` using CDP `Target.closeTarget` and accepting a target id, tab dict, or omitted current target.
  - `62894f2`: added `domain-skills/hubspot/private-app-webhooks.md`.
  - `2fa1b1e`: moved that skill to `agent-workspace/domain-skills/hubspot/private-app-webhooks.md` and removed task-specific wording.
- Rust migration decisions:
  - Added `META_CLOSE_TAB` and a daemon `close_tab` meta handler that calls `Target.closeTarget`, defaults to the current attached target when `target_id` is omitted, clears stale attachment/dialog state for closed current tabs, and best-effort reattaches to another real page.
  - Exposed `close-tab` through `bhrun`, the top-level `browser-harness` facade, `bh-wasm-host` manifest/config, and `bh_guest_sdk::close_tab`.
  - Extended tab smoke coverage and the tab-response Rust guest to close temporary tabs after verification.
  - Mapped the HubSpot upstream domain skill into `domains/hubspot/private-app-webhooks.md`; legacy upstream roots are represented by the `domains/` mapping convention documented in `domains/README.md`, not duplicated as `domain-skills/` or `agent-workspace/domain-skills/` directories.
  - Updated tab usage docs, Python subprocess wrapper examples, and README snippets for `close-tab`.

## Daily Sync Verification Evidence — 2026-05-21

- `cargo fmt --manifest-path rust/Cargo.toml --all -- --check` passed.
- `cargo check --manifest-path rust/Cargo.toml --workspace` passed.
- `env -u CFLAGS -u CC cargo test --manifest-path rust/Cargo.toml --workspace` passed.
- `env -u CFLAGS -u CC cargo run --quiet --manifest-path rust/Cargo.toml --bin bhrun -- summary` passed.
- `env -u CFLAGS -u CC cargo run --quiet --manifest-path rust/Cargo.toml --bin browser-harness -- --help` passed and exposes the Rust facade command list.
- `git diff --check` passed.
- `scripts/scan_sensitive.sh` still requires Bash 4 `mapfile`; a macOS-compatible Python equivalent of the same regex checks passed with no obvious secrets or local path leaks.

## Daily Upstream Sync — 2026-05-21

- Fetched `origin/main` and `upstream/main`; local `main` started clean and equal to `origin/main`.
- Previous target: `ea7d1710ba8621c658d6d61fe46bcf77746e83e4`; new upstream target: `9da5ec2e52a30ed74752366d89075cbc3821a445`.
- New upstream range `ea7d1710ba8621c658d6d61fe46bcf77746e83e4..9da5ec2e52a30ed74752366d89075cbc3821a445`: 2 non-merge commits.
- Upstream changes analyzed:
  - `ae83151`: deleted stale top-level `domain-skills/amazon/cart.md` and `domain-skills/amazon/orders.md`.
  - `ad7f4f2`: removed Firecrawl mentions from `agent-workspace/domain-skills/facebook/groups.md` and `agent-workspace/domain-skills/facebook/pages.md`, switching to vendor-neutral downstream-extractor language.
- Rust migration decisions:
  - Deleted `domains/amazon/cart.md` and `domains/amazon/orders.md` to match the upstream cleanup; these files were mapped from the legacy `domain-skills/` path.
  - Updated `domains/facebook/groups.md` and `domains/facebook/pages.md` to remove Firecrawl references with vendor-neutral phrasing, matching the upstream semantic changes.

## Daily Upstream Sync — 2026-05-22

- Fetched `origin/main` and `upstream/main`; local `main` started clean and equal to `origin/main`.
- Previous target: `9da5ec2e52a30ed74752366d89075cbc3821a445`; new upstream target: `6d20866664ea3d9691b27bbf64f42ae097437dc3`.
- New upstream range `9da5ec2e52a30ed74752366d89075cbc3821a445..6d20866664ea3d9691b27bbf64f42ae097437dc3`: 2 commits (1 non-merge commit + 1 merge).
- Upstream changes analyzed:
  - `1583bd7c0b98629bfabcfd6e61051138de9495f1`: added `agent-workspace/domain-skills/qbo/report-export.md` for QuickBooks Online custom report PDF export.
- Rust migration decisions:
  - Mapped the upstream domain skill to `domains/qbo/report-export.md` following the `domains/` convention.
  - No Rust code changes were needed because this is a documentation-only domain skill.

## Daily Sync Verification Evidence — 2026-05-22

- `cargo fmt --manifest-path rust/Cargo.toml --all -- --check` passed.
- `cargo check --manifest-path rust/Cargo.toml --workspace` passed.
- `env -u CFLAGS -u CC cargo test --manifest-path rust/Cargo.toml --workspace` passed.
- `env -u CFLAGS -u CC cargo run --quiet --manifest-path rust/Cargo.toml --bin bhrun -- summary` passed.
- `env -u CFLAGS -u CC cargo run --quiet --manifest-path rust/Cargo.toml --bin browser-harness -- --help` passed.
- `git diff --check` passed.
- `scripts/scan_sensitive.sh` still requires Bash 4 `mapfile` on macOS `/bin/bash`; a macOS-compatible Python equivalent of the same regex checks passed with no obvious secrets or local path leaks.

## Daily Upstream Sync — 2026-06-15

- Fetched `origin/main` and `upstream/main`; local `main` started clean and equal to `origin/main`.
- Previous target: `6d20866664ea3d9691b27bbf64f42ae097437dc3`; new upstream target: `2cfaa7ea4c77b17b4c2434403865fa4b6d637b29`.
- New upstream range `6d20866664ea3d9691b27bbf64f42ae097437dc3..2cfaa7ea4c77b17b4c2434403865fa4b6d637b29`: 5 non-merge commits plus merge commits on `upstream/main`.
- Upstream changes analyzed:
  - `f20e4aa` / PR #443: Added plugin manifest and skill files for agent marketplaces (`.claude-plugin/marketplace.json`, `.claude-plugin/plugin.json`, `skills/browser-harness/SKILL.md`, `skills/browser-harness/references/install.md`).
  - `fdad2e5`: Updated `.claude-plugin` to use Claude marketplace source format.
  - `7b01296`: Reverted PR #443 (the add-plugin-manifest merge).
  - `2baa4a2`: Re-added plugin manifest and skill as canonical, no-drift source of truth (same 4 files).
  - `5421622`: Removed `.grok-plugin` manifest, keeping only the Claude Code `.claude-plugin/` and `skills/` entries.
- Net upstream effect: 4 new documentation/plugin-manifest files for Claude Code agent marketplace integration.
- Rust migration decisions:
  - Created `.claude-plugin/marketplace.json` adapted for the Rust fork: repo URL points to `yangchuansheng/browser-harness-rust`, description mentions Rust-native CLI.
  - Created `.claude-plugin/plugin.json` adapted for the Rust fork: author/URLs reference `yangchuansheng/browser-harness-rust`, keywords include `rust`.
  - Created `skills/browser-harness/SKILL.md` adapted for the Rust fork: CLI commands use JSON-heredoc format (`browser-harness page-info <<'JSON'...`), references `domains/` instead of upstream `agent-workspace/domain-skills/`, all operations mapped to `browser-harness` Rust CLI subcommands.
  - Created `skills/browser-harness/references/install.md`: installation uses `cargo run -- install` + `$CARGO_HOME/bin` instead of upstream pip/uv; clone URL is the Rust fork.
  - No Rust code changes were needed — all changes are documentation/plugin-manifest only.
  - No Python runtime files were copied; no domain-skill files were added or modified.

## Daily Sync Verification Evidence — 2026-06-15

- `cargo fmt --manifest-path rust/Cargo.toml --all -- --check` passed.
- `cargo check --manifest-path rust/Cargo.toml --workspace` passed.
- `env -u CFLAGS -u CC cargo test --manifest-path rust/Cargo.toml --workspace` passed.
- `env -u CFLAGS -u CC cargo run --quiet --manifest-path rust/Cargo.toml --bin bhrun -- summary` passed.
- `env -u CFLAGS -u CC cargo run --quiet --manifest-path rust/Cargo.toml --bin browser-harness -- --help` passed.
- `git diff --check` passed.
- `scripts/scan_sensitive.sh` still requires Bash 4 `mapfile` on macOS `/bin/bash`; a macOS-compatible Python/rg scan passed with no obvious secrets or local path leaks in tracked/unignored files.

## Daily Upstream Sync — 2026-06-21

- Fetched `origin/main` and `upstream/main`; local `main` started clean and equal to `origin/main`.
- Previous target: `2cfaa7ea4c77b17b4c2434403865fa4b6d637b29`; new upstream target: `a606cf773d3f9553fd56dee9638cd7de34d3b765`.
- New upstream range `2cfaa7ea4c77b17b4c2434403865fa4b6d637b29..a606cf773d3f9553fd56dee9638cd7de34d3b765`: 2 README-only commits.
- Upstream changes analyzed:
  - `a5d7a18`: updated `README.md` with Browser Use Cloud promotion copy.
  - `b03f199`: updated `README.md` with the final Browser Use Cloud promotion copy.
- Net upstream effect: inserted a Browser Use Cloud link near the top of `README.md` before the setup prompt context.
- Rust migration decisions:
  - Added the same Browser Use Cloud promotion sentence to the Rust fork `README.md` after the opening description and before the Rust-specific capability overview.
  - No Rust code changes were needed because the upstream range is documentation-only.
  - No Python runtime files were copied.

## Daily Sync Verification Evidence — 2026-06-21

- `cargo fmt --manifest-path rust/Cargo.toml --all -- --check` passed.
- `cargo check --manifest-path rust/Cargo.toml --workspace` passed.
- `env -u CFLAGS -u CC cargo test --manifest-path rust/Cargo.toml --workspace` passed.
- `env -u CFLAGS -u CC cargo run --quiet --manifest-path rust/Cargo.toml --bin bhrun -- summary` passed.
- `env -u CFLAGS -u CC cargo run --quiet --manifest-path rust/Cargo.toml --bin browser-harness -- --help` passed.
- `git diff --check` passed.
- `./scripts/scan_sensitive.sh` could not run because `rg` is not installed in this cron environment; a Python fallback using the script's exact regex rules passed with no obvious secrets or local path leaks.

## Daily Upstream Sync — 2026-06-26

- Fetched `origin/main` and `upstream/main`; local `main` started at `304d28d5adbc2ac25d2af59850cda3b5b12b0ede`, equal to `origin/main` before the sync.
- Previous target: `a606cf773d3f9553fd56dee9638cd7de34d3b765`; new upstream target: `7594909e7963c9ba328e39cc79e9f20ff94b2a82`.
- New upstream range `a606cf773d3f9553fd56dee9638cd7de34d3b765..7594909e7963c9ba328e39cc79e9f20ff94b2a82`: 12 non-merge commits plus release workflow and packaging changes.
- Upstream changes analyzed:
  - Added release-ready Python packaging, root `browser-harness` wrapper, release workflow, and package version bumps through `0.1.3`.
  - Added Browser Use Cloud auth storage with `auth login`, `auth status`, and `auth logout` flows.
  - Added opt-out telemetry support and docs for telemetry state.
  - Clarified remote daemon/cloud browser flow and install/update guidance.
  - Hardened IPC/admin paths and packaged skill/install docs.
- Rust migration decisions:
  - Kept the Rust workspace and installer as the packaging source of truth; bumped the Rust workspace package version to `0.1.3` instead of copying Python packaging or release workflow files.
  - Added Rust-native Browser Use auth storage in `bh-remote`, using `BROWSER_USE_API_KEY` first and a private JSON auth file under the Browser Harness config directory second.
  - Exposed `browser-harness auth status`, `browser-harness auth login --api-key-stdin`, JSON `auth login`, and `browser-harness auth logout` through `bhctl` and the top-level facade.
  - Updated cloud browser docs and `SKILL.md` so agents can authenticate once without keeping the API key in every environment.
  - Did not copy Python runtime files or external telemetry POST behavior; no Rust telemetry network path was added.

## Daily Sync Verification Evidence — 2026-06-26

- `cargo fmt --manifest-path rust/Cargo.toml --all -- --check` initially reported rustfmt-only changes; `cargo fmt --manifest-path rust/Cargo.toml --all` was run and the follow-up check passed.
- `cargo check --manifest-path rust/Cargo.toml --workspace` passed.
- `env -u CFLAGS -u CC cargo test --manifest-path rust/Cargo.toml --workspace` passed.
- `env -u CFLAGS -u CC cargo run --quiet --manifest-path rust/Cargo.toml --bin bhrun -- summary` passed.
- `env -u CFLAGS -u CC cargo run --quiet --manifest-path rust/Cargo.toml --bin browser-harness -- --help` passed and lists `auth` as an admin command.
- `BH_CONFIG_DIR=/tmp/browser-harness-rust-auth-smoke env -u CFLAGS -u CC cargo run --quiet --manifest-path rust/Cargo.toml --bin browser-harness -- auth status` passed with `status: missing` and the override auth path.
- `git diff --check` passed.
- `./scripts/scan_sensitive.sh` could not run because `rg` is not installed in this cron environment; a Python fallback using the script's exact regex rules passed with no obvious secrets or local path leaks.

## Daily Upstream Sync — 2026-07-02

- Fetched `origin/main` and `upstream/main`; local `main` started clean and equal to `origin/main`.
- Previous target: `7594909e7963c9ba328e39cc79e9f20ff94b2a82`; new upstream target: `4d75f115c039bf769d614fbd8d996a961e143567`.
- New upstream range `7594909e7963c9ba328e39cc79e9f20ff94b2a82..4d75f115c039bf769d614fbd8d996a961e143567`: 6 commits (all non-merge).
- Upstream changes analyzed:
  - `5d34276`: Renamed "browser-harness" to "browser-use" in `SKILL.md` frontmatter/description/title, `pyproject.toml` (version 0.1.3→0.1.4), and `tests/unit/test_skill.py`.
  - `ffa5db0`: Updated `test_skill.py` metadata description to match new name.
  - `be7a36d`: Aligned skill identity with harness CLI in `SKILL.md`.
  - `81daf7f`: Restored browser-use skill identity in `SKILL.md`.
  - `057dd15`: Added v4 cloud agent promotion link to `SKILL.md`.
  - `607f168`: Updated auth key-importation example in `SKILL.md` from bare `--api-key-stdin` to `printf '%s' "$BROWSER_USE_API_KEY" | browser-harness auth login --api-key-stdin`.
- Net upstream effect: Full rebranding from "browser-harness" to "browser-use" in skill metadata, v4 cloud promotion, and auth key-importation doc update. No Python runtime code changes.
- Rust migration decisions:
  - Updated root `SKILL.md` frontmatter: `name: browser-use`, description with "Always use browser-use..." prefix, title to `# Browser Use`.
  - Added v4 cloud promotion paragraph (`cloud.browser-use.com?utm_source=skill&...`) to the remote browsers section of root `SKILL.md`.
  - Updated `skills/browser-harness/SKILL.md` frontmatter to match (name/description/title) while preserving Rust fork-specific CLI documentation.
  - Bumped workspace version in `rust/Cargo.toml` from `0.1.3` to `0.1.4`.
  - Did not copy `pyproject.toml` or `tests/unit/test_skill.py` (Python-only packaging).
  - Did not rename the Rust CLI binary from `browser-harness`; the binary/CLI name remains `browser-harness` for compatibility.
  - No Python runtime files were copied; no Rust code logic changed.

## Daily Sync Verification Evidence — 2026-07-02

- `cargo fmt --manifest-path rust/Cargo.toml --all -- --check` passed.
- `cargo check --manifest-path rust/Cargo.toml --workspace` passed.
- `env -u CFLAGS -u CC cargo test --manifest-path rust/Cargo.toml --workspace` passed (178 tests, 0 failures).
- `env -u CFLAGS -u CC cargo run --quiet --manifest-path rust/Cargo.toml --bin bhrun -- summary` passed.
- `env -u CFLAGS -u CC cargo run --quiet --manifest-path rust/Cargo.toml --bin browser-harness -- --help` passed.
- `git diff --check` passed.
- `./scripts/scan_sensitive.sh` could not run because `rg` is not installed in this cron environment; a Python fallback using the script's exact regex rules passed with no new secrets or local path leaks (all hits were pre-existing public Metacritic API keys and localhost CDP examples common across docs/tests).

## Daily Upstream Sync — 2026-07-08

- Fetched `origin/main` and `upstream/main`; local `main` started clean and equal to `origin/main`.
- Previous target: `4d75f115c039bf769d614fbd8d996a961e143567`; new upstream target: `12e3152e5254a5e304e3bedcfa90be7d27906360`.
- New upstream range `4d75f115c039bf769d614fbd8d996a961e143567..12e3152e5254a5e304e3bedcfa90be7d27906360`: 5 non-merge commits + 3 merge commits.
- Upstream changes analyzed:
  - `20d3cbd`: Renamed skill identity from "browser-use" back to "browser-harness" in `SKILL.md` and `tests/unit/test_skill.py`.
  - `0db04a8`: Major telemetry rework — added helper step tracing, output tail capture, detached telemetry sender subprocess, agent client detection, richer `capture_cli_event`, and `ANONYMIZED_TELEMETRY` env support.
  - `edaecbc`: Added cloud browser nudge text to `SKILL.md` (when to proactively suggest cloud browsers for concurrency and captcha avoidance).
  - `4301714`: Added `daemon_browser_kind()` to admin module and `BROWSER_KIND` to daemon, reporting "cloud"/"cdp"/"local" in ping responses for telemetry.
  - `b4da250`: Guarded daemon browser-kind ping behind telemetry enable check.
- Rust migration decisions:
  - Reverted root `SKILL.md` frontmatter from `name: browser-use` to `name: browser-harness`, description from "Always use browser-use" to "Always use browser-harness", and title from "# Browser Use" to "# browser-harness".
  - Reverted `skills/browser-harness/SKILL.md` frontmatter identically.
  - Added cloud browser nudge paragraph to root `SKILL.md` remote browsers section.
  - Added `browser_kind()` method to `DaemonConfig` that classifies as "cloud" (BU_BROWSER_ID), "cdp" (BU_CDP_WS/BU_CDP_URL), or "local" (default).
  - Included `browser_kind` in the daemon ping response alongside `pong` and `pid`, matching upstream's self-reported browser type.
  - Did not add Rust telemetry network calls — the fork continues to not send telemetry POSTs. The `ANONYMIZED_TELEMETRY` env is already documented in the telemetry opt-out docs; agent client detection constants and detached sender mechanics were not ported because the fork has no telemetry POST path.
  - Did not copy Python runtime files or `tests/unit/test_skill.py`.
  - No version bump needed — the workspace version stays at `0.1.4`.

## Daily Sync Verification Evidence — 2026-07-08

- `cargo fmt --manifest-path rust/Cargo.toml --all -- --check` passed (after rustfmt run).
- `cargo check --manifest-path rust/Cargo.toml --workspace` passed.
- `env -u CFLAGS -u CC cargo test --manifest-path rust/Cargo.toml --workspace` passed (169 tests, 0 failures).
- `env -u CFLAGS -u CC cargo run --quiet --manifest-path rust/Cargo.toml --bin bhrun -- summary` passed.
- `env -u CFLAGS -u CC cargo run --quiet --manifest-path rust/Cargo.toml --bin browser-harness -- --help` passed.
- `git diff --check` passed.
- `./scripts/scan_sensitive.sh` could not run because `rg` is not installed in this cron environment; a Python fallback using the script's exact regex rules passed with no new secrets or local path leaks (all 31 hits were pre-existing public UUIDs in domain docs and test UUIDs).
