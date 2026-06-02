# PR collision canonical selection

Use this case when two or more PRs target the same issue, user-facing behavior,
or source path and the queue needs one maintainer-selected implementation.

Anchor case: `nexu-io/open-design#3298` and `#3052`, both targeting `#1581`
with a WSL folder-open fix.

## Trigger

- Two PRs reference the same issue or bug.
- The file sets overlap in the load-bearing path.
- The intended behavior is materially the same.
- Merging both would duplicate logic, split behavior, or make one branch
  conflict/supersede the other.

This is not a `duplicate-title` case unless the title rule also fires. Treat it
as a maintainer selection problem, not an author mistake.

## Read order

1. Pull live metadata for each PR:

   ```bash
   gh pr view <num> --json state,reviewDecision,mergeStateStatus,statusCheckRollup,labels,files,latestReviews,comments,headRefOid
   ```

2. Compare the changed file names:

   ```bash
   gh pr diff <num> --name-only
   ```

3. Read the focused patch only when the metadata does not settle the choice:

   ```bash
   gh pr diff <num> --patch --color never
   ```

## Selection facts

Prefer the PR that is already closest to the repository's merge gate:

- `state=OPEN`, `mergeStateStatus=CLEAN`, required checks complete.
- Latest human review is `APPROVED` or the remaining review concern is clearly
  addressed in the current head.
- The implementation covers the known failure modes from prior review.
- Tests cover the red-green seam and the edge cases named by reviewers.
- The patch fits the existing architecture and leaves the narrowest follow-up.

Do not choose purely by creation time. Earlier PRs can still shape the selected
solution, but review state and resolved blockers decide the canonical branch.

## Author communication

When the unselected PR is closed or already closed, keep the public comment
short and factual:

- Thank the author for moving the issue forward.
- State that another PR is the selected path to avoid duplicate fixes.
- Name the concrete technical reason, especially resolved blocker coverage.
- Say whether this PR's direction aligned with the final shape.
- Point experienced contributors toward an appropriate area, not beginner
  issues, when their history shows they are already comfortable in the repo.

Example adapted from the anchor case:

```text
@<author> thanks for picking up #<issue> and pushing the <behavior> direction forward. I compared this with #<selected> since both PRs route that path toward <outcome>.

Keeping this one closed to avoid duplicating the fix. The current head here was still blocked on <reviewer>'s <specific blocker>; #<selected> carries the same direction and adds the missing coverage for <edge cases>.

Your PR helped confirm the right fix shape, so this was still useful work even though we're landing the other branch. Thanks again for getting this moving. If you feel like picking up another one, focused bugfixes around <matching area> have been a strong fit for your recent contributions.
```

## Anchor outcome

For `#3298` vs `#3052`:

- `#3052` was `CLOSED`, `CHANGES_REQUESTED`, and `BLOCKED`; review said the
  WSL helper still lacked a safe fallback and matching regression coverage.
- `#3052` covered native pass-through and the WSL happy path only.
- `#3298` was `OPEN`, `CLEAN`, `APPROVED`, and green; it covered the same WSL
  to `wslpath -w` to `explorer.exe` direction, plus fallback coverage for
  `wslpath`, `explorer.exe`, and post-launch non-zero Explorer exits.
- The maintainer-duty decision was to treat `#3298` as the canonical
  implementation and leave `#3052` closed with a short author-facing note.
