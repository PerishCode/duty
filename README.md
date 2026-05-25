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
duty facts --limit 10
duty facts --format json
duty list --limit 20
duty list --lane docs --format json
duty classify 2856
duty classify --all --limit 20 --json
duty view 2856
duty view 2856 --json
duty help
```

The default checkout config in `duty.json` points at `nexu-io/open-design`.
Pass `--repo owner/name` or use another `duty.json` to target a different repo.

Successful live queue reads are cached under `.tmp/duty/cache/<owner>__<repo>/`.
When live GitHub reads fail, `duty queue` falls back to the latest usable cache.
`--offline` skips GitHub and reads cache only.

`duty facts` is the parity-work intake surface. It fetches the PR metadata,
stats, files, reviews, commits, comments, and assignment event streams that
later `list`, `classify`, `view`, and `assignment` commands will consume. Chunk
failures are reported as warnings when a partial snapshot can still be built.

`duty list` classifies a facts snapshot into the first `tools-pr list` parity
slice: path-derived lanes, review-state buckets, labels, bot-only approval
detection, and lane/bucket/author/draft filters.

`duty classify` emits factual script-level tags from the same facts snapshot.
The first parity slice covers label, rebase, forbidden surface, duplicate-title,
bot-only approval, stale approval, maintainer-edit, unresolved
changes-requested, non-ASCII design-system slug, org-member, and
awaiting-response tags. `classify --all` writes
`.tmp/duty/classify/<stem>.json`; pass `--print` to also dump the full report to
stdout. Full-queue reports include GraphQL rate-limit telemetry when live
GitHub reads are available.

`duty view` fetches a single PR and emits the first `tools-pr view` parity
slice: lane and boundary facts, denoised top files, validation hints, recent
human review/comment context, CI rollup, and PR body preview. Successful live
views are cached under `.tmp/duty/cache/<owner>__<repo>/views/`; `--offline`
reads that cache.

## Development

```bash
python3 scripts/init.py
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
cargo run --locked -p duty-cli -- help
cargo run --locked -p duty-cli -- facts --limit 2
cargo run --locked -p duty-cli -- list --limit 5
cargo run --locked -p duty-cli -- classify --all --limit 5 --json
cargo run --locked -p duty-cli -- view 2856
```

## Scope

`duty` is personal workflow automation. It should stay read-only by default and
keep product-specific policy out of upstream product repositories.

For source-change shape, see [AGENTS.md](./AGENTS.md).
