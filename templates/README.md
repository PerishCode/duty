# templates

Voice and style references for recurring PR-duty artifacts.

- `agent-review.md`, `awaiting-author-nudge.md`, and `duplicate-title-ask.md`
  are aesthetic references. Absorb their beats and tone, then compose a fresh
  artifact for each PR. Do not `sed`-substitute the placeholders and post the
  rendered text verbatim.
- `examples/` holds frozen historical exemplars of the agent-review style
  applied to three different PR shapes — scope-expanded, clean contract
  feature, CHANGES_REQUESTED with prior human reviews. They teach how shape
  varies with the PR, not the live state of those PRs.

This directory holds the **style** layer of the three-layer PR-duty stack:

- conduct rules (factual output, language detection, channel split): see
  `../AGENTS.md` (`PR-duty Conduct`).
- per-tag dictionary and operational handbook (detection rules, minimum
  actions, escalation timing): see `../docs/pr-duty-playbook.md`.
- voice and section beats for the artifacts those steps produce: this
  directory.
