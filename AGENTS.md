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
  fill-in forms; do not post them verbatim. See `templates/README.md` for the
  navigation between the three layers (conduct rules, per-tag playbook, style
  references).
- `scripts/init.py` is the idempotent post-clone initializer. It quick-fails on
  missing required tools or repository entrypoints, installs local hooks, and
  exits cleanly only when the checkout is ready for development.
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

Two distinct paths share the workspace:

- **Hot path** — real PR-duty work runs the installed `duty` binary. It is
  cwd-independent, has no compile latency, and matches the actual end-user
  experience.
- **Iteration path** — changing `duty` itself runs `cargo …` against the
  workspace.

Keep them separate: use `cargo run -p duty-cli -- <sub>` only when validating
a code change to that subcommand, not for real triage.

### Install / upgrade

```bash
cargo install --locked --path crates/duty-cli
```

This places `duty` in `~/.cargo/bin/`. Re-run after pulling new commits or
changing local source. A flavor-aligned binary distribution pipeline will
replace this entrypoint later.

### PR-duty hot path

```bash
duty queue --limit 5
duty queue --limit 5 --format json
duty facts --limit 5
duty list --limit 5
duty classify --all --limit 5 --json
duty view 2856
duty assignment --limit 5
```

### Workspace iteration

```bash
python3 scripts/init.py
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
cargo run --locked -p duty-cli -- help
```

`python3 scripts/init.py` is the default post-clone command. Use `--force`
only when intentionally replacing existing non-init hooks; the script backs
them up first.

## PR-duty Conduct

These rules govern every PR-duty artifact produced in this workspace, whether
the author is a human maintainer or an agent: classify reports, view briefs,
internal agent review drafts, GitHub comments, and IM nudges. The CLI itself
is built to satisfy them by default; the rules apply to anyone composing on
top of CLI output.

### Factual output

Every line in a PR-duty artifact must be either:

- a data observation — a fact from `gh`, the diff, or repository paths; or
- a direct citation — a hard rule from a referenced document (this repo's
  `AGENTS.md`, the consumer repo's review or contribution guidelines, etc.)
  with the source noted inline.

Do not emit risk verdicts (`LOW` / `MEDIUM` / `HIGH`), merge recommendations,
or directive language (`should`, `must`, `do not`, `recommended`,
`encouraged`, `suggested`). Judgment belongs to the reviewer who consumes the
artifact, not to the tool or the agent.

### Templates are style references

`templates/*.md` and `templates/examples/*.md` capture tone and section beats
for recurring artifacts. They are not fill-in forms:

- read the template to absorb tone and beats,
- compose a fresh artifact woven around the PR-specific facts,
- vary wording across PRs handled in the same session — identical text
  broadcast across a contributor's notifications breaks the human-to-human
  voice.

### Author-addressed vs broadcasting artifacts

- **Author-addressed**: nudges, duplicate-title asks, close-with-reason
  comments — anything `@`-mentioning a specific contributor in a private-feeling
  exchange. Detect the author's preferred language before writing:

  ```bash
  gh pr view <num> --json body,comments,author --jq '
    .author.login as $a |
    ([.body, (.comments[] | select(.author.login == $a) | .body)] | join("\n"))
  '
  ```

  If the resulting text contains CJK characters, write in Chinese; otherwise
  English.

- **Broadcasting**: PR descriptions, commit messages, review summaries visible
  to all reviewers. English regardless of author.

### Org-member channel split

The `org-member` tag is informational. When it co-occurs with operational
tags (`awaiting-*`, `duplicate-title`, `maintainer-edits-disabled`, etc.),
route those operational communications through the team's internal IM, not
as a GitHub comment. Substantive review feedback and the final merge decision
stay on GitHub regardless.

Every operational playbook step that posts a public comment must filter
`org-member` out first. See `docs/pr-duty-playbook.md` for the canonical
filters.

### Per-tag operations

`docs/pr-duty-playbook.md` is the per-tag dictionary and operational handbook:
each classify-emitted tag, its detection rule, the minimum action it invites,
and escalation timing. The conduct rules above apply across every step in the
playbook.

## PR-duty Triage Approach

A triage round runs by bucket, not by PR. The classify report is the entry
panel; each bucket maps to one operation in `docs/pr-duty-playbook.md`. Work
through whichever buckets matter for the round and stop. The PR-duty Conduct
rules above apply to every artifact emitted along the way.

### Tool roles

- `duty classify --all --json` writes the per-tag bucket index to
  `.tmp/duty/classify/<timestamp>.json`. Re-run at the start of every round —
  the queue moves while you read.
- `duty view <num>` is the per-PR structural brief from the REST-style cache.
  Use when a bucket entry needs a closer look before action.
- `gh pr view --json …` + `jq` is the live sanity-check at every decision
  point (direct merge, enqueue, comment, close). The classify report and the
  `view` cache both lag the live state by seconds to minutes; treat them as
  read-only inputs, not as the source of truth at write time.
- `templates/*.md` are tone and beats only. Read; do not paste.
- `.tmp/duty/reviews/<num>.md` are agent-review briefs for bucket-3 PRs.
  Default path: surface to the maintainer for routing, no public posting.
  Escalated path (`docs/pr-duty-playbook.md` §`Agent public pre-review`): the
  same brief is also posted as a GitHub `--approve` or `--comment` review
  when the eligibility gate is met.

### Per-bucket cadence

Buckets imply different work-per-PR costs. Plan the round around how many of
each there are:

- Direct-merge candidates (`bot-only-approval` strictly intersected with
  surgical — size/XS, single file, ≤ ~30 lines, no boundary or contract
  surface): 5–10 min each, the fastest output of a round.
- Public-comment buckets (`duplicate-title`,
  `awaiting-author-response-24h` ≥ 96h after `org-member` filter): 5–15 min
  each, because composing a fresh message per PR is mandatory.
- Offline buckets (`awaiting-reviewer-response-24h`,
  `awaiting-first-review-24h`): no GitHub output, but require a maintainer
  decision (claim the review or re-ping a reviewer).
- Signal-only buckets (`needs-rebase`, `unresolved-changes-requested`,
  `stale-approval`): no per-PR action; they route PRs into the buckets above.
- Bucket-3 PRs (contract-lane, large refactor, security-sensitive,
  scope-mixed): the most expensive — an agent-review brief per PR, surfaced
  to the maintainer. When the eligibility gate in
  `docs/pr-duty-playbook.md` §`Agent public pre-review` is met, the same
  brief is posted publicly as a GitHub review instead of staying local.
- `author-cluster`: a structural signal, not a per-PR action. When the tag
  fires (≥ 7 open PRs from one author), the cohort is handled as a single
  consolidated brief at `.tmp/duty/reviews/author-cluster-<login>.md`;
  per-PR actions on the same PRs are suppressed for that round to avoid
  broadcast-style nudges.

See `docs/pr-duty-playbook.md` for the per-tag thresholds, filters, and
commands.

### Author-cluster batching

When several PRs from the same author land in the same operational bucket
(common in `unresolved-changes-requested`), read all of them in one pass
before deciding per-PR actions. Avoids re-paging the same author context.

### Pre-write sanity check

Every GitHub write action — `gh pr merge`, `gh pr comment`, `gh pr close`, the
merge-queue `enqueuePullRequest` mutation — re-runs the relevant sanity check
immediately before invocation. The classify report and the `view` cache are
inputs to planning the round, never the source of truth at the moment of
writing.

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
