# Phase 6 Review Rubric

Last audited: 2026-04-19

Use this rubric for manual review of the two active phase-6 questions.

## Default-Lane Review Labels

Every lead candidate should be scored with one primary label:

- `helpful_primary`
  - belonged in the top one to three slots and improved repair direction
- `helpful_secondary`
  - true and useful, but not the best top action
- `correct_but_low_value`
  - structurally true, but unlikely to improve the immediate repair
- `correct_but_not_fixable_now`
  - true, but not actionable in the current patch
- `distracting`
  - crowded out a more causal or fixable lead
- `wrong`
  - unsupported or materially overstated

## `large_file` Review Labels

Review `large_file` with the same label set, but capture these notes explicitly:

- did it point to a concrete containment or split move
- was the file part of the changed surface
- did the patch itself worsen the pressure
- did the signal crowd out a more causal lead
- was the suggested repair packet concrete enough to act on

## Family Selection Questions

When reviewing the default-lane family experiment, answer:

- did the family deserve one of the one to three primary slots
- did the family help more than it distracted
- did the family improve actionability or only structural correctness

## `large_file` Questions

When reviewing the `large_file` experiment, answer:

- should `large_file` stay default-lane eligible
- if yes, is it unconditional or guardrailed
- if guardrailed, what is the narrowest rule that preserves its value

## Evidence To Record

For each reviewed run, capture:

- top-action follow
- top-action help
- task success
- patch expansion cost
- reviewer disagreement
- which stronger lead would have won if the reviewed lead was distracting

## Promotion Discipline

Do not keep a lead family in the default lane just because it is often true.

Keep it only if:

- it is followed often enough to matter
- it helps often enough to justify occupying a primary slot
- it does not create avoidable patch sprawl or review friction
