# duty

Personal maintainer-duty automation CLI.

`duty` keeps queue intake, cache-backed GitHub reads, and future review-duty
helpers out of upstream product repositories. The first command focuses on open
PR queue visibility with a degraded path for GitHub API instability: JSON via
`gh` first, plain `gh pr list` second, cache last.

## Install

Unix:

```bash
./install.sh
```

Windows PowerShell:

```powershell
.\install.ps1
```

Both scripts install the local checkout with `cargo install --locked --path
crates/duty-cli`.

## Usage

```bash
duty queue
duty queue --repo nexu-io/open-design --limit 10
duty queue --format json
duty queue --offline
duty help
```

The default checkout config in `duty.json` points at `nexu-io/open-design`.
Pass `--repo owner/name` or use another `duty.json` to target a different repo.

Successful live queue reads are cached under `.tmp/duty/cache/<owner>__<repo>/`.
When live GitHub reads fail, `duty queue` falls back to the latest usable cache.
`--offline` skips GitHub and reads cache only.

## Development

```bash
python3 scripts/init.py
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
cargo run --locked -p duty-cli -- help
```

## Scope

`duty` is personal workflow automation. It should stay read-only by default and
keep product-specific policy out of upstream product repositories.

For source-change shape, see [AGENTS.md](./AGENTS.md).

