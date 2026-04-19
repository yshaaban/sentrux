# Phase 6 Promotion Ledger

Last audited: 2026-04-19

This ledger records the current standing of the two active phase-6 questions.

## Active Questions

| Question | Status | Current read | Next required evidence |
| --- | --- | --- | --- |
| Default-lane family selection | Screening in progress | core causal families look strongest, but the winning mix is not yet proven across all three repos | screening evidence on `sentrux`, `parallel-code`, and `one-tool`, then confirmation shortlist |
| `large_file` admissibility | Screening in progress | `large_file` is still a serious retained candidate, but its admissibility rule is unresolved | compare always-eligible, guardrailed, and supporting-only variants across all three repos |

## Family Standing

| Family | Standing | What would promote it | What would demote it |
| --- | --- | --- | --- |
| clone and clone-followthrough | strong candidate | continues to win top-action help across repos | loses to more direct obligation or boundary signals on contained tasks |
| obligation and incomplete propagation | strong candidate | remains concrete and patch-local in repair packets | broad sibling hunts overwhelm fix surfaces |
| boundary and rule surfaces | strong candidate | continues to produce clear fix-first leads | becomes review-noisy relative to direct task goals |
| patch-local concentration | candidate with guardrails | improves help without broad structural noise | behaves like generic maintainability pressure |
| `large_file` | active candidate, unresolved | improves help or task success without distracting from more causal leads | stays correct-but-low-value or increases patch expansion |
| broad structural pressure beyond `large_file` | suspect | would need clear repair leverage beyond watchpoint value | continues to act as maintainer context rather than immediate intervention |

## Decision Recording Rule

A row should move from candidate to keep, constrain, or demote only when:

- the experiment tracker shows screening and confirmation evidence
- the decision is written with [decision-template.md](./decision-template.md)
- the decision cites measured changes against `current_policy`
