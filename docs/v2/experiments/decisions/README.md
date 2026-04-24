# Experiment Decision Records

Last audited: 2026-04-24

This directory stores durable keep, constrain, demote, and park decisions for v2 completion gates.

Do not mark a phase `Completed` in [../../completion-execution-tracker.md](../../completion-execution-tracker.md) unless the required decision record exists here and cites the checked-in evidence that satisfies the phase exit gate.

## Filename Format

```text
YYYY-MM-DD-<phase>-<question-or-family>-<decision>.md
```

Examples:

```text
2026-04-24-phase-6-large-file-constrain.md
2026-04-24-phase-6-default-lane-family-selection-keep.md
2026-04-24-phase-3-llm-adjudication-park.md
```

## Decision Values

| Decision | Meaning |
| --- | --- |
| `keep` | Promote or retain the signal, product rule, or surface for the stated rollout scope |
| `constrain` | Retain only under a named admissibility rule or guardrail |
| `demote` | Remove from default-on or default-lane behavior while possibly keeping as supporting context |
| `park` | Stop active execution until a named dependency or evidence gap closes |

## Required Record Shape

Copy this shape into each decision record and replace every placeholder.

```markdown
# <Phase> <Question Or Family> <Decision>

Date: YYYY-MM-DD

## Decision

- decision: keep | constrain | demote | park
- phase:
- scope:
- product effect: default-on | default-lane | supporting-only | maintainer-lane | parked
- rollout scope:

## Control

- control:
- treatment or winning variant:
- losing variants:
- repo/task scope:

## Evidence

- primary artifacts:
- generated run outputs:
- review artifacts:
- regression guard:

## Outcome Read

- primary metric movement:
- top-action help:
- top-action follow:
- task success:
- escaped regression:
- reviewer disagreement:
- patch expansion:

## Decision Rationale

- what improved:
- what stayed weak:
- what was correct but low-value:
- what was distracting:

## Guardrails

- admissibility rule:
- verification surfaces:
- what not to over-generalize:
- evidence that would reopen this decision:

## Tracker Update

- tracker row changed:
- previous status:
- new status:
- remaining open gate:
```

## Evidence Rules

- Cite repository paths for all checked-in docs, specs, schemas, fixtures, scorecards, and proof artifacts.
- Cite `.sentrux/evals/...` output paths when the decision depends on generated evidence.
- Do not cite private screenshots, local notes, or field anecdotes as decisive evidence.
- Use `current_policy` as the control for phase-6 product decisions. Use `no_intervention` only when reading session-arm baseline results inside a paired live task battery.
