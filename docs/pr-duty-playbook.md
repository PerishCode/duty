# PR-duty playbook

Tag dictionary and per-tag operational handbook for the `duty` PR-duty surface.
The conduct rules in `../AGENTS.md` (`PR-duty Conduct`) apply to every artifact
produced while executing this playbook.

The dictionary is the contract between `duty classify` output and reviewer
action: each tag has a single mechanical rule and a single data source. New
tags require the rule to be expressible as one factual sentence, derivable
purely from `gh` data plus file paths, and unlikely to false-positive in
legitimate use.

## Tag dictionary

| Tag | Rule | Data source |
| --- | --- | --- |
| `bot-only-approval` | `reviewDecision === "APPROVED"` and every review with `state === "APPROVED"` is bot-authored | gh.reviewDecision + latestReviews |
| `needs-rebase` | `mergeStateStatus ∈ {DIRTY, BEHIND}` | gh.mergeStateStatus |
| `forbidden-surface` | A touched path matches the consumer repo's forbidden-surface regex set | files + consumer-repo lane config |
| `unlabeled` | The PR is missing at least one of the `size/`, `risk/`, `type/` label prefixes | gh.labels |
| `duplicate-title` | Another open PR by the same author has a byte-for-byte identical `title` | cross-PR title index |
| `non-ascii-slug` | A design-system root touched by the PR has a slug failing `/^[a-z0-9-]+$/` | files + consumer-repo lane config |
| `maintainer-edits-disabled` | `maintainerCanModify === false` | gh.maintainerCanModify |
| `org-member` | PR author's GitHub login appears in `gh api orgs/<owner>/members` | gh REST orgs members list |
| `unresolved-changes-requested` | A reviewer's latest review has `state === "CHANGES_REQUESTED"` (primary); falls back to `reviewDecision === "CHANGES_REQUESTED"` when no per-reviewer CR survives the latest-per-author reduction | gh.latestReviews[].state · gh.reviewDecision |
| `stale-approval` | An `APPROVED` review's `commit.oid` differs from current `headRefOid` | gh.latestReviews[].commit.oid + gh.headRefOid |
| `awaiting-author-response-24h` | Latest human-reviewer signal is newer than the latest author signal and is ≥ 24h ago | latestReviews + comments + commits |
| `awaiting-reviewer-response-24h` | Latest author signal is newer than the latest human-reviewer signal, ≥ 24h ago, and at least one human-reviewer signal exists | latestReviews + comments + commits |
| `awaiting-first-review-24h` | No human review or non-author non-bot comment exists, and `createdAt` is ≥ 24h ago | latestReviews + comments + createdAt |
| `author-cluster` | A single author has ≥ 7 open PRs in the current snapshot. Threshold is fixed in code (`AUTHOR_CLUSTER_THRESHOLD`). Each PR in the cluster gets `tag.clusterSize` set to the snapshot-wide open-PR count for that author | cross-PR open-PR-count index by author |

**Signal-time definitions** used by the three `awaiting-*` tags:

- *author signal* = `max(commits[].committedDate) ∪ max(comments[?author.login == prAuthor].createdAt)`
- *human-reviewer signal* = `max(latestReviews[?author != prAuthor && !bot].submittedAt) ∪ max(comments[?author != prAuthor && !bot].createdAt)`

The three `awaiting-*` tags are mutually exclusive by construction. Each one
sets `tag.awaitingHours` — the integer hour count between the awaiting-window
start (latest reviewer signal, latest author signal, or `createdAt`
respectively) and the classify-run moment. Downstream consumers sort PRs
within an awaiting bucket by `awaitingHours`, or floor-divide by 24 for days.

## Operational playbook

Each row is the minimum action; escalation (close, force-merge, etc.) stays
with the maintainer. Every step that posts a public comment must filter
`org-member` out first per the channel split in `AGENTS.md`.

### Direct merge (APPROVED + CLEAN, surgical)

1. Sanity-check the merge state:

   ```bash
   gh pr view <num> --json state,reviewDecision,mergeStateStatus,statusCheckRollup \
     --jq '{state, reviewDecision, mergeStateStatus,
            checks: [.statusCheckRollup[] | {conclusion, name: (.workflowName // .name)}]}'
   ```

   Expected: `state=OPEN`, `reviewDecision=APPROVED`, `mergeStateStatus=CLEAN`,
   every check `SUCCESS`.

2. If `duty classify <num>` includes `bot-only-approval`, verify the change is
   surgical (size/XS, single file, < ~30 lines, no boundary or contract
   surface) before proceeding. The surgical judgment lives outside `duty`.

3. Squash-merge per repo convention:

   ```bash
   gh pr merge <num> --squash --delete-branch
   ```

   **Fallback for merge-queue repos with disabled auto-merge** (currently
   `nexu-io/open-design`): the command above fails with
   `GraphQL: Auto merge is not allowed for this repository
   (enablePullRequestAutoMerge)` because the branch-protection merge queue
   is enabled while the `enablePullRequestAutoMerge` mutation is disabled at
   the repo level. Add the PR to the queue directly — the queue then runs
   the squash and the branch deletion:

   ```bash
   PR_NODE_ID=$(gh pr view <num> --repo <owner/repo> --json id --jq '.id')
   gh api graphql \
     -f query='mutation($pr: ID!) {
       enqueuePullRequest(input: { pullRequestId: $pr }) {
         mergeQueueEntry { position estimatedTimeToMerge }
       }
     }' -F pr="$PR_NODE_ID"
   ```

   The mutation only succeeds when the PR already satisfies the queue
   preconditions verified in step 1 (`state=OPEN`,
   `reviewDecision=APPROVED`, `mergeStateStatus=CLEAN`, all required checks
   `SUCCESS`). If it returns an error, re-check the sanity output rather
   than retrying.

4. Confirm:

   ```bash
   gh pr view <num> --json state,mergedAt,mergeCommit \
     --jq '{state, mergedAt, sha: .mergeCommit.oid[0:10]}'
   ```

### `duplicate-title`

1. Inspect both PRs to choose the older / more-iterated one — the author may
   want to preserve its history:

   ```bash
   gh pr view <num> --json number,headRefName,commits,additions,deletions,createdAt,updatedAt
   ```

2. Read `../templates/duplicate-title-ask.md` for tone and beats, compose a
   fresh comment for the actual PR pair (author login, both branch names,
   both commit counts, both diff sizes), and post on the older PR:

   ```bash
   gh pr comment <older-num> -F /tmp/dup-ask-<older-num>.md
   ```

3. If no response after 7 days, close the older PR with a superseded note:

   ```bash
   gh pr close <older-num> --comment "Superseded by #<newer-num>."
   ```

### `awaiting-author-response-24h`

Nudge only after 96 hours (≥ 4 days). For the 24h–96h window, hold off; the
author may still be drafting.

1. Filter the classify report for the threshold cohort, exclude `org-member`,
   sort by `awaitingHours` descending:

   ```bash
   jq '[.byTag["awaiting-author-response-24h"][] as $n
        | .byNumber[($n|tostring)] as $tags
        | select($tags | map(.name) | contains(["org-member"]) | not)
        | $tags[]
        | select(.name == "awaiting-author-response-24h" and .awaitingHours >= 96)
        | {n: $n, h: .awaitingHours}]
       | sort_by(-.h)' .tmp/duty/classify/<latest>.json
   ```

2. For each remaining PR, read `../templates/awaiting-author-nudge.md` for
   tone, compose a fresh comment that weaves in the author login and a
   human-formatted awaiting duration. Vary wording across PRs nudged in the
   same session.

3. Post:

   ```bash
   gh pr comment <num> -F /tmp/nudge-<num>.md
   ```

4. Re-check the classify report in a follow-up run; the awaiting tag clears
   once the author responds. If no response by 14 days, escalate (a more
   direct stale-warning or close-after-warning).

### `awaiting-reviewer-response-24h`

Reviewer-side delay. No public nudge — the action is for the maintainer to
either pick up the review themselves or re-ping a reviewer offline.

### `awaiting-first-review-24h`

No reviewer engagement yet. Same offline routing: claim the review locally or
solicit one through the team's normal review-assignment channel.

### `author-cluster`

`author-cluster` is a structural signal, not a per-PR action item. When a
single author shows up with 7+ open PRs, the playbook's other rows are
suppressed for that cohort: a 7+ nudge or 7+ close-supersede sequence
broadcasts in a way that violates the conduct rules, regardless of how
correct each individual action would be in isolation.

1. List the cluster from the latest classify report:

   ```bash
   jq -r '.byAuthor | to_entries[] | select(.value | length >= 7)
          | "\(.key) (\(.value | length)): \(.value | join(", #") | "#" + .)"' \
     .tmp/duty/classify/<latest>.json
   ```

2. Cross-reference each PR's other tags (`needs-rebase`,
   `unresolved-changes-requested`, `awaiting-*`, `bot-only-approval`, etc.) to
   understand the cluster's shape: is it stalled, drifting, mostly mergeable,
   mostly contested?

3. Write a single consolidated brief to
   `.tmp/duty/reviews/author-cluster-<login>.md`. The brief is a transient
   runtime artifact for the maintainer; it is never posted to GitHub. Beats:
   per-PR one-line state, recurring patterns, the maintainer-level question
   ("triage these as a batch?", "ask the author to consolidate?",
   "fast-forward the mergeable subset?").

4. If `org-member` also fires for the same author, route the brief through
   the team's internal IM. Otherwise hand the brief to the maintainer and
   let them choose the next step. Do not act per-PR off this tag.

### `org-member` plus operational tags

When `org-member` co-occurs with `awaiting-*`, `duplicate-title`,
`maintainer-edits-disabled`, or other operational tags, route the conversation
through the team's internal IM instead of GitHub. Substantive review feedback
and the final merge decision remain on GitHub regardless of org membership.

### Agent review (bucket-3 PRs)

For high-value or high-risk technical PRs — contract-lane PRs, large
refactors, security-sensitive fixes, scope-mixed PRs flagged via classify —
an agent produces an internal analysis brief:

1. Pull `duty view <num>` for the structural brief and `gh pr diff <num>` for
   the patch.
2. Read `../templates/agent-review.md` for tone and section pool, compose for
   the specific PR. Sections appear only when they carry signal.
3. Write the brief to `.tmp/duty/reviews/<num>.md`. This is a transient
   runtime artifact, never posted directly to GitHub.
4. Surface the brief to the maintainer; the decision to split, block, merge,
   IM, or post a public review stays with them.

`../templates/examples/*.md` are frozen historical exemplars of the
agent-review style applied to three PR shapes — scope-expanded, clean
contract feature, and CHANGES_REQUESTED with prior human reviews. They
illustrate how section shape varies with the PR. Treat them as style
references, not canonical scaffolding.
