# duty-cli

Owns the installable `duty` binary, config discovery, GitHub command execution,
cache IO, and report output.

## Rules

- Keep commands read-only by default. Any GitHub side effect must be an explicit
  command with the behavior documented in root `AGENTS.md`.
- Keep command parsing small and predictable; update `help_text()` and tests
  when flags move.
- Prefer live GitHub JSON, then plain `gh` output, then cache when a command can
  still produce useful information.
- Keep shelling out to `gh` localized to `github.rs`.
- Keep cache paths under `.tmp/duty/` by default.

## Tests

`tests/unit.rs` mounts source modules via `#[path = "../src/<file>.rs"]` for
pure parser and option tests. Do not add tests that require a live GitHub token
to the default suite.

## Commands

```bash
cargo run --locked -p duty-cli -- help
cargo run --locked -p duty-cli -- queue --limit 5
cargo test --locked -p duty-cli
```

