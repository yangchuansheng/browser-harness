# Contributing to Browser Harness Rust

Pull requests and improvements are welcome. Bug fixes, documentation changes,
focused runtime improvements, and domain skills are all useful.

## Development

From a checkout, run the current Rust working tree through Cargo:

```bash
cargo run --quiet --manifest-path rust/Cargo.toml --bin browser-harness -- page-info <<'JSON'
{"daemon_name":"default"}
JSON
```

The installed `browser-harness` command exercises the installed binary layout.
Agent-facing documentation should use that installed command; repository
development examples may use the Cargo invocation above.

Before opening a pull request, run:

```bash
cargo fmt --manifest-path rust/Cargo.toml --all -- --check
cargo test --manifest-path rust/Cargo.toml --workspace
```

## Domain skills

Domain skills teach agents selectors, flows, APIs, waits, and edge cases they
would otherwise have to rediscover.

- Let observed browser behavior drive each contribution.
- Add the focused site knowledge under [`domains/<site>/`](domains/).
- Keep contributions small and focused.
- Browse existing examples such as `domains/github/`, `domains/linkedin/`, and
  `domains/amazon/` for the expected shape.

Open an issue when the right contribution boundary needs discussion.
