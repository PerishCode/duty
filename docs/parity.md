# tools-pr parity notes

`duty` is the standalone maintainer-duty home for the personal workflow that
used to live in Open Design `tools-pr`. The default command surface stays
read-only.

## Matched command surfaces

| tools-pr | duty | Status |
| --- | --- | --- |
| `tools-pr list` | `duty list` | Matched: lane, bucket, author, draft filters, text/JSON output. |
| `tools-pr view <num>` | `duty view <num>` | Matched: lane/boundary facts, denoised top files, validation hints, human review/comment summary, CI rollup, body preview, JSON output. |
| `tools-pr classify <num>` | `duty classify <num>` | Matched: script tags, `--json`, org-member hydration when live GitHub reads are available. |
| `tools-pr classify --all` | `duty classify --all` | Matched: full report artifact, optional stdout print, GraphQL rate telemetry. Duty writes under `.tmp/duty/classify/`. |
| `tools-pr assignment` | `duty assignment` | Matched: assignee grouping, assigned/idle timing, blockers/status from classify tags, `--user`, `--unassigned`, `--include-drafts`, JSON output. |

## Intentional path differences

- Runtime cache and artifacts use `.tmp/duty/` instead of `.tmp/tools-pr/`.
- `duty` accepts `--repo owner/name` and a checkout-local `duty.json`; it is not
  tied to the current Git repository.
- `duty` keeps degraded cache-backed behavior where practical. If live GitHub
  reads fail, facts/list/classify/view/assignment can still use cached snapshots
  when available.
- `duty classify --all --json` is treated as a compatibility alias for
  `--print`: it still writes the report artifact and also prints JSON.

## Migrated workflow references

- Comment/review style references live in `templates/`.
- Internal agent review briefs should be written under `.tmp/duty/reviews/`.
- The templates remain aesthetic references, not fill-in forms or automated
  GitHub comments.

## Remaining before tools-pr retirement discussion

- Run one representative live queue comparison between `pnpm tools-pr ...` and
  `duty ...` when GitHub's GraphQL path is healthy enough for `tools-pr`.
- Decide whether Open Design-specific lane/rule text should stay bundled in
  `duty` or move behind a repo adapter/config layer.
- Draft the Open Design cleanup plan separately; do not remove `tools-pr` until
  the user explicitly approves that phase.
