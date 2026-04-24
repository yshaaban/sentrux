# Completion Execution Model

Last audited: 2026-04-24

This document defines how the v2 master plan moves from `In progress` to `Completed`.

It is intentionally operational. A phase closes only when checked-in implementation, checked-in evidence, and a written decision record all satisfy the gate for that phase.

## Source Of Truth

- Program status lives in [../completion-execution-tracker.md](../completion-execution-tracker.md).
- Detector and ranking work items live in [../comprehensive-detection-upgrade-plan.md](../comprehensive-detection-upgrade-plan.md).
- Active phase-6 experiment specs live in [../evals/experiments](../evals/experiments).
- Human experiment decisions use [decision-template.md](./decision-template.md) until a phase-specific record exists.
- The deterministic gate evaluator lives in [../../../scripts/evals/build-completion-gates.mjs](../../../scripts/evals/build-completion-gates.mjs) and fails closed when required phase decision records are absent.

The tracker may summarize evidence, but it must not promote a phase without a durable decision record under this directory.

## Completion Rubric

Score every phase against the same six dimensions.

| Dimension | Required for completion | Fails completion when |
| --- | --- | --- |
| Product contract | The phase output preserves the agent-lane versus maintainer-lane split and the one-to-three primary action budget | New surfaces create a warning wall or blur patch repair with broad repo-health commentary |
| Implementation | The repo contains the implementation, configuration, schema, or fixture artifacts required by the phase | The phase is represented only by docs, local experiments, or uncommitted artifacts |
| Evidence | Checked-in proof artifacts show the intended behavior on the fixed repo/task matrix or the phase-specific fixture corpus | Evidence exists only as anecdote, single-run inspection, or private/untracked output |
| Outcome lift | The phase improves top-action help, task success, regression avoidance, repair-packet clarity, or false-positive pressure against the relevant control | Metrics improve only detector count, raw recall, or structural truth without agent repair value |
| Decision record | A written keep, constrain, demote, or park decision cites the decisive evidence and remaining guardrails | The tracker changes status without a dated decision record |
| Regression guard | The phase has a repeatable validation path that can be rerun before release | The behavior depends on manual judgment that cannot be reproduced or reviewed |

Completion is binary. A phase with five passing dimensions and one missing dimension remains `In progress`.

## Status Semantics

- `Planned`: the repo may contain inputs or precursors, but no shipping workflow satisfies the phase intent.
- `In progress`: implementation and/or evidence exists, but at least one rubric dimension is incomplete.
- `Completed`: all rubric dimensions pass and the phase exit gate below has a decision record.
- `Parked`: the phase remains valid, but the active program has intentionally stopped advancing it until a named dependency closes.
- `Demoted`: the phase output is intentionally excluded from default-on or default-lane behavior because evidence showed negative or insufficient value.

`Parked` and `Demoted` are not failure states. They are valid product decisions when supported by evidence.

## Evidence Hierarchy

Use the strongest applicable evidence available.

| Rank | Evidence type | Use |
| --- | --- | --- |
| 1 | Paired treatment-versus-baseline run on the fixed repo/task matrix | Default-on promotion, release gating, ranking policy changes |
| 2 | Signal-matched experiment with screening and confirmation stages | Default-lane family selection and admissibility guardrails |
| 3 | Reviewed session corpus plus evidence review output | Promotion candidates, demotion candidates, false-positive pressure |
| 4 | Fixture-backed detector proof with regression tests | Engine correctness for narrow issue families |
| 5 | Public-safe proof snapshot or golden output | Release communication and integration stability |
| 6 | Manual field review | Backlog seeding only; never sufficient for completion by itself |

When evidence ranks conflict, the higher-ranked evidence wins unless the decision record explains why the lower-ranked evidence is more applicable to the exact phase question.

## Phase Exit Gates

### Phase 0: Reset The Product Contract

Exit gate:

- Doctrine, roadmap, scorecard policy, session verdicts, evidence review, and product surfaces share one lane vocabulary.
- The default patch surface exposes the same top-action contract used by the evaluation artifacts.
- No compatibility shim or fallback field is required for current scorecards, session corpus, or findings consumers.
- Decision record confirms that downstream consumers can rely on lane/default-on metadata as canonical.

Minimum evidence:

- Fresh scorecard, session corpus, and evidence review artifacts using the same lane metadata.
- Product-surface inspection showing the same lane split in `check`, findings, and repair packets.

### Phase 1: Harden The Intervention-Grade Signal Set

Exit gate:

- Default lane remains dominated by causal, patch-local, repairable families.
- Watchpoints do not occupy the lead lane unless patch-worsened and repairable.
- False-positive pressure stays below the agreed release threshold for repeated use.
- Decision record lists each kept, constrained, and demoted family.

Minimum evidence:

- Signal-matched experiment or paired run across `sentrux`, `parallel-code`, and `one-tool`.
- Reviewed examples for clone, obligation, boundary, library-evolution, and patch-local concentration families.

### Phase 2: Expand The Semantic Obligation Graph

Exit gate:

- Changed DTO, config, registry, command, public API, test, and doc surfaces produce bounded sibling-surface guidance.
- Findings name concrete missing consumers, likely fix sites, and verification surfaces.
- Changed-symbol precision is good enough that obligation findings remain patch-local.
- Decision record identifies which obligation families are default-lane eligible, supporting-only, or parked.

Minimum evidence:

- Fixture proof for each promoted obligation family.
- Repo-local run showing no warning-wall regression from broader obligation discovery.
- Reviewed repair packets demonstrating clear sibling and verification surfaces.

### Phase 3: Add Bounded LLM Adjudication

Exit gate:

- LLM use is limited to deterministic evidence bundles.
- Audit logs capture input hash, output hash, model, latency, decision, and cited evidence IDs.
- Static-plus-adjudication beats static-only on the selected outcome metric.
- Decision record states the allowed production role: advisory-only, rerank, suppress, repair guidance, or parked.

Minimum evidence:

- Paired static-only versus static-plus-adjudication comparison.
- Audit-log sample proving reproducibility.
- Cost and latency readout for the evaluated path.

### Phase 4: Add Checker And Pattern Synthesis

Exit gate:

- Confirmed incidents are clustered into candidate detector patterns.
- Synthesized checks are generated from evidence, not intuition.
- Held-out validation proves the synthesized checks catch useful incidents without unacceptable false positives.
- Decision record lists accepted checks, rejected checks, and parked pattern classes.

Minimum evidence:

- Checked-in incident clustering artifact.
- Candidate checker output.
- Held-out false-positive and incident-recall review.

### Phase 5: Build The Treatment-Vs-Baseline Evidence Program

Exit gate:

- Fixed tasks can be run as paired treatment and baseline sessions.
- Per-task, per-signal, and per-repo effect sizes are reported.
- Promotion and demotion candidates are derived from outcome metrics, not anecdote.
- Decision record confirms at least one stable lane where treatment beats baseline.

Minimum evidence:

- Reproducible paired runs on the fixed repo/task matrix.
- Scorecard, session corpus, telemetry summary, and evidence review from the same run set.
- Explicit readout for top-action help, top-action follow, task success, escaped regression count, and reviewer disagreement.

### Phase 6: Product Surface Compression

Exit gate:

- Default lane consistently shows one to three primary repair actions.
- Every primary action has a complete repair packet.
- Default-lane family selection and `large_file` admissibility have keep, constrain, or demote decisions.
- Decision record updates the phase-6 promotion ledger and cites measured changes against `current_policy`.

Minimum evidence:

- Screening and confirmation evidence for the two active phase-6 questions.
- Reviewed runs across `sentrux`, `parallel-code`, and `one-tool`.
- Explicit crowd-out analysis for `large_file` and other structural pressure.

### Phase 7: Release Gate

Exit gate:

- Public-safe proof artifacts refresh cleanly.
- Release hygiene and public preflight pass.
- Default-on signals have completed promotion records.
- Release checklist references the evidence bundle that justifies default-on behavior.

Minimum evidence:

- Passing public preflight and hygiene outputs.
- Fresh public-safe proof snapshot.
- Completed decision records for all default-on promoted signals.

## Promotion And Demotion Decision Records

Every promotion, constraint, demotion, or parking decision gets one Markdown file under:

```text
docs/v2/experiments/decisions/
```

Use this filename format:

```text
YYYY-MM-DD-<phase>-<question-or-family>-<decision>.md
```

Examples:

```text
2026-04-24-phase-6-large-file-constrain.md
2026-04-24-phase-1-boundary-keep.md
2026-04-24-phase-3-llm-adjudication-park.md
```

Required fields:

| Field | Requirement |
| --- | --- |
| Decision | `keep`, `constrain`, `demote`, or `park` |
| Phase | Master-plan phase number and name |
| Scope | Signal family, experiment question, detector class, or product surface |
| Control | Usually `current_policy`; use `no_intervention` only for session-arm baseline reads |
| Evidence | Paths to checked-in artifacts and generated run outputs |
| Primary outcome | Metric movement that decided the result |
| Secondary outcomes | Supporting precision, packet, disagreement, and patch-expansion reads |
| Product effect | Default-on, default-lane, supporting-only, maintainer-lane, or parked |
| Guardrails | Narrow admissibility rule and what would reopen the decision |
| Tracker update | Exact status or progress row changed after the decision |

Decision records must include enough detail for a later maintainer to reproduce why a signal was promoted or demoted without re-reading the full experiment history.

## Progress Tracking

The tracker should carry one progress row per phase with these fields:

| Field | Meaning |
| --- | --- |
| status | Current phase status from the status semantics above |
| implementation checkpoint | Highest checked-in implementation state that is already true |
| evidence checkpoint | Strongest checked-in evidence currently available |
| open gate | The next concrete condition blocking completion |
| next proof artifact | The next file or run output expected to close the gate |
| decision record | Existing or required decision file |

Progress rows must be updated when:

- a new checked-in implementation changes a phase checkpoint
- a new run produces stronger evidence
- a decision record keeps, constrains, demotes, or parks a signal
- a phase status changes

Do not update a phase from `In progress` to `Completed` in the tracker unless the corresponding decision record exists.

## Current Required Decision Records

No completion decision records are currently checked in for the active completion gates.

The next records required are:

| Priority | Required record | Blocks |
| --- | --- | --- |
| 1 | Phase 6 default-lane family selection keep/constrain/demote record | Phase 1 and Phase 6 completion |
| 2 | Phase 6 `large_file` admissibility keep/constrain/demote record | Phase 6 completion and release default-lane policy |
| 3 | Phase 5 treatment-versus-baseline confirmation record | Phase 5 and Phase 7 completion |
| 4 | Phase 0 lane-contract canonicalization record | Phase 0 completion |
| 5 | Phase 3 LLM adjudication advisory/park/rerank record | Phase 3 completion |
