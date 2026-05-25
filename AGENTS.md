# AGENTS

`duty` is a personal maintainer-duty automation CLI.

It owns personal queue triage, resilient GitHub data collection, cache-backed
review preparation inputs, and small maintainer workflow helpers. It does not
own product semantics for the repositories it inspects, repository-specific
merge policy, or automated GitHub side effects by default.

## Directory Rules

### Repository Shape

- `crates/` contains the Rust workspace crates. Each core crate owns its local
  `AGENTS.md`; read the child file before editing that subtree.
- `.github/workflows/` contains CI workflows.
- `docs/` contains parity notes and maintainer-facing workflow documentation.
- `templates/` contains maintainer voice/style references for recurring
  PR-duty comments and internal agent review artifacts. They are references, not
  fill-in forms; do not post them verbatim.
- `scripts/init.py` is the idempotent post-clone initializer. It quick-fails on
  missing required tools or repository entrypoints, installs local hooks, and
  exits cleanly only when the checkout is ready for development.
- `install.sh` and `install.ps1` are the public local installation entrypoints
  at the repository root.
- `duty.json` is the default personal config for this checkout. Consumer repos
  can keep their own config files outside this source tree.

### Recursive AGENTS Index

- `crates/duty-cli/AGENTS.md`: installable `duty` binary, config discovery,
  command parsing, GitHub command execution, cache IO, and report output.
- `crates/duty-core/AGENTS.md`: shared duty models and pure parsers.

When adding or removing a core subtree, update this index in the same change.
Child `AGENTS.md` files should stay local: ownership, directory shape, commands,
workflow notes, and FAQ for that subtree.

### Project Boundaries

- Keep the default CLI read-only on GitHub. Commands that approve, comment,
  merge, close, assign, or push must be explicit subcommands with dry-run or
  confirmation shape documented beside the implementation.
- Keep repository-specific product rules in data/config or dedicated adapters,
  not in generic GitHub fetch primitives.
- Prefer small degraded outputs over all-or-nothing queue intake. If GitHub API
  JSON flakes, keep a plain-output or cache fallback where the command can still
  be useful.
- Keep personal workflow voice in docs/templates, not in upstream product repos.

## Common Commands

```bash
python3 scripts/init.py
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
cargo run --locked -p duty-cli -- queue --limit 5
cargo run --locked -p duty-cli -- queue --limit 5 --format json
cargo run --locked -p duty-cli -- facts --limit 5
cargo run --locked -p duty-cli -- list --limit 5
cargo run --locked -p duty-cli -- classify --all --limit 5 --json
cargo run --locked -p duty-cli -- view 2856
cargo run --locked -p duty-cli -- assignment --limit 5
```

`python3 scripts/init.py` is the default post-clone command. Use `--force` only
when intentionally replacing existing non-init hooks; the script backs them up
first.

## Standard Workflow

### Initialize

After cloning or when hooks look stale, run:

```bash
python3 scripts/init.py
```

The generated hooks contain their concrete actions directly. The pre-commit hook
currently runs fmt, clippy, tests, a help-command smoke check, shell syntax
checks, and PowerShell syntax checks when `pwsh` is available. The commit-msg
hook validates the commit subject shape.

### Branch Names

Use `<area>/<kebab-case-slug>`, where `<area>` matches the touched crate or
concern. Examples:

- `cli/queue-cache-fallback`
- `github/rest-hydration`
- `docs/init-workflow`

### Commit Messages

Subject: `<area>: <imperative summary>` on one line, ideally <= 72 characters.
The body explains why the change is shaped this way first, then the change list.
End with any `Co-Authored-By:` trailers when pair-coded or agent-assisted.

### Tests

Unit tests for `duty-cli` live under `crates/duty-cli/tests/<area>.rs` and are
registered in `crates/duty-cli/tests/unit.rs`:

```rust
#[path = "../src/<file>.rs"]
mod <module>;

mod <area>_cases;
```

Pure shared model/parser tests for `duty-core` live under
`crates/duty-core/tests/`.

### Pre-PR Checks

Every PR must pass these commands before review:

```bash
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
cargo run --locked -p duty-cli -- help
```

CI reruns them across Linux, Windows, and macOS.

### PR Descriptions

Use these top-level sections, in order:

```markdown
## Why
<what is broken or missing today>

## What
<concrete change list; reference filenames and modules>

## Tests
<commands run and results>
```

Add `## Compatibility` when an output shape, config field, or exit-code behavior
moves. Add `## Trade-off worth flagging` when the change has a downside that
reviewers should hold in mind.

### Merging

`main` is PR-only and protected by the `guard` workflow. Required approvals are
intentionally `0`; the guard matrix is the merge gate.

After opening a non-draft PR, default to enabling repository auto-merge:

```bash
gh pr merge <num> --auto --squash --delete-branch
```

Do not add workflow files just to auto-enable auto-merge. If auto-merge cannot
be enabled or the repository disables merge commits, wait for green checks and
fall back to the smallest equivalent manual command, usually
`gh pr merge <num> --squash --delete-branch`.

## FAQ

### Why is this separate from upstream product repos?

Maintainer-duty queueing and personal review workflows tend to carry individual
triage style, local cache policy, and private operating habits. Keeping those in
`duty` avoids making product repositories maintain that personal layer.

### Does `duty` perform GitHub side effects?

Not by default. The initial `queue` command is read-only and cache-backed.
Future side-effecting commands should be explicit and documented as such.

### Where do repository-specific rules go?

Start with config or a dedicated adapter module. Do not put Open Design-specific
policy directly into generic GitHub fetch or cache primitives.
