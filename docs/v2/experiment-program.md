# Sentrux V2 Experiment Program

Last audited: 2026-04-24

## Purpose

This document narrows the checked-in experiment program to the only two active product questions that currently matter:

1. which signal families belong in the default lane
2. whether `large_file` belongs in the default lane, and under what exact admissibility rule

Everything else stays parked until these two questions are answered with treatment evidence.

## Active Scope

The checked-in experiment registry is now phase-6 only.

Take forward:

- the existing experiment infrastructure
- `current_policy` as the control arm across repos
- the fixed repo set already encoded in the checked-in manifests:
  - `sentrux`
  - `parallel-code`
  - `one-tool`

Park for later:

- obligation breadth expansion
- bounded LLM adjudication
- broader treatment-vs-baseline rollout decisions beyond the two phase-6 questions

Those tracks are still real product concerns, but they are not active experiment questions until the default lane itself is trustworthy.

## Execution Surfaces

The active program uses these checked-in surfaces:

- machine-readable specs under [./evals/experiments](./evals/experiments)
- the schema at [./evals/experiment.schema.json](./evals/experiment.schema.json)
- the runner at [../../scripts/evals/run-experiment.mjs](../../scripts/evals/run-experiment.mjs)
- the tracker builder at [../../scripts/evals/build-experiment-tracker.mjs](../../scripts/evals/build-experiment-tracker.mjs)
- the fixed repo manifests under [./evals/repos](./evals/repos)
- the machine-readable task matrix at [./evals/phase-6-repo-task-matrix.json](./evals/phase-6-repo-task-matrix.json)
- the fixed task battery documented in [./experiments/phase-6-repo-task-matrix.md](./experiments/phase-6-repo-task-matrix.md)
- the review rubric in [./experiments/phase-6-review-rubric.md](./experiments/phase-6-review-rubric.md)
- the decision ledger in [./experiments/phase-6-promotion-ledger.md](./experiments/phase-6-promotion-ledger.md)

The phase-6 task matrix intentionally reuses the existing live and replay batch manifests from the treatment-baseline evidence lane. That avoids maintaining two copies of the same repo/task battery while keeping the active product questions narrower than the broader phase-5 evidence program.

## Program Rules

Every active phase-6 experiment must follow these rules:

1. Keep `current_policy` unchanged as the control arm while a screening or confirmation cycle is running.
2. Use the fixed repo set of `sentrux`, `parallel-code`, and `one-tool`.
3. Reuse the checked-in live and replay task battery instead of inventing ad hoc repo tasks mid-cycle.
4. Keep the default lane budget at one to three primary actions.
5. Judge signals by repair outcomes, not by structural truth alone.
6. Treat `large_file` as a retained candidate that must earn or lose default-lane status from evidence, not from prior bias.
7. Do not activate parked workstreams in the registry until one of the active phase-6 questions is resolved.

## Control Semantics

Two different controls exist and should not be conflated:

- `current_policy`
  - the experiment-level control variant used in the checked-in experiment specs
- `no_intervention`
  - the session-arm baseline used inside the live task battery when a batch compares intervention prompts

Phase-6 product decisions should be written against `current_policy`. Session-arm results remain supporting evidence inside that comparison.

## Fixed Repo Matrix

The three checked-in repos serve different purposes:

- `sentrux`
  - dogfood realism
  - ranking and packet quality on the product repo itself
- `parallel-code`
  - clone, boundary, followthrough, and structural replay pressure
- `one-tool`
  - mixed-language public-safe repo with command-surface and export-followthrough pressure

The detailed task mapping lives in [./experiments/phase-6-repo-task-matrix.md](./experiments/phase-6-repo-task-matrix.md).

## Current Honest Read

The experiment infrastructure is now good enough to carry the narrowed program, but the product question is still open:

- the default lane is more coherent than before, but it is not yet proven to improve repair behavior consistently
- `large_file` remains contested, but it is simple enough and often useful enough that it should be tested as a serious retained candidate rather than presumed demotion
- the active work should now concentrate on default-lane family mix and `large_file` admissibility only

Current implementation checkpoint:

- JS report selection and the Rust MCP agent brief now both consume the same default-lane policy cap, eligible sources, and per-kind guardrails.
- The generated experiment tracker now reports variant run coverage, metric means, paired `current_policy` comparisons, evidence state, and primary-metric deltas.
- `session_end` finding surfaces now carry repair packets, so review and agent loops can measure repairability on introduced findings instead of only on promoted actions.
- The current generated tracker still reports `fresh_runs_required` for both active experiments. The infrastructure is ready, but no phase-6 product decision should be made until fixed-matrix run artifacts exist for `sentrux`, `parallel-code`, and `one-tool`.

## Active Questions

### Question 1: Default-Lane Family Selection

Goal:

- determine which signal families consistently earn one of the one to three primary slots

Decision standard:

- a family belongs in the default lane only if it improves repair behavior, not just reviewed precision

### Question 2: `large_file` Default-Lane Admissibility

Goal:

- determine whether `large_file` should stay in the default lane, and if so under what guardrails

Decision standard:

- `large_file` stays only if it adds repair value without crowding out more causal actions

## Stage Model

Each active experiment tracks three stages:

### Screen

Goal:

- compare variants cheaply and eliminate weak policies early

Required outcome:

- identify the shortlist worth confirmation

### Confirm

Goal:

- verify that the shortlist still wins on the fixed repos and fixed task battery

Required outcome:

- one variant or guardrail set materially outperforms the others on primary outcomes

### Decide

Goal:

- convert measured results into a product rule

Required outcome:

- a recorded keep, constrain, or demote decision with explicit evidence

## Experiment A: Default-Lane Family Ablation

Spec:

- [./evals/experiments/default-lane-family-ablation.json](./evals/experiments/default-lane-family-ablation.json)

Question:

- which family mix produces the best one-to-three action lane

Control:

- `current_policy`

Screening variants:

- core causal families only
- core causal families plus patch-local concentration
- core causal families plus `large_file` as the only structural pressure family
- current policy with `large_file` elevated only when patch-worsened and repairable

What this experiment decides:

- which families remain eligible for the default lane
- whether structural pressure should survive there beyond `large_file`

## Experiment B: `large_file` Default-Lane Admissibility

Spec:

- [./evals/experiments/large-file-default-lane-admissibility.json](./evals/experiments/large-file-default-lane-admissibility.json)

Question:

- does `large_file` help enough to stay in the default lane, and what guardrail is required

Control:

- `current_policy`

Screening variants:

- always eligible
- changed-file only
- patch-worsened only
- concrete containment only
- supporting-only

What this experiment decides:

- keep unchanged
- keep with guardrails
- demote to supporting

## Metrics That Decide The Program

Primary metrics:

- `top_action_help_rate`
- `top_action_follow_rate`
- `task_success_rate`
- `patch_expansion_rate`
- `intervention_net_value_score`

Secondary metrics:

- `ranking_miss_count`
- `reviewed_precision`
- `reviewer_disagreement_rate`
- `repair_packet_complete_rate`
- `repair_packet_fix_surface_clear_rate`
- `repair_packet_verification_clear_rate`
- `remediation_success_rate`

`large_file` specific readouts:

- how often it reaches the top one to three slots
- how often it is followed
- how often it is helpful when followed
- how often it is correct but low-value
- how often it distracts from a more causal lead

## Decision Rules

### Default-Lane Family Decisions

Keep a family in the default lane only if:

- it repeatedly survives into the one to three primary slots
- following it improves repair outcomes
- it does not increase patch expansion or reviewer disagreement enough to offset the gain

### `large_file` Decisions

Keep `large_file` in the default lane if:

- it improves help rate or task success against control
- it does not materially crowd out more causal leads
- the repair packet makes the containment move concrete enough to act on

Constrain `large_file` if:

- value is real but concentrated in changed-file, patch-worsened, or containment-ready cases

Demote `large_file` if:

- it is often correct but low-value
- it increases patch sprawl
- it repeatedly displaces a more causal lead

## What Counts As Progress

The tracker should move in this order:

1. screening variants defined and dry-run clean
2. generated tracker shows every active variant and paired `current_policy` comparison
3. all control-arm repo runs present
4. screening evidence captured on all three repos
5. confirmation shortlist recorded
6. decision recorded in the ledger

The stage tracker is not complete when a spec exists. It is complete only when the checked-in tracker, run artifacts, and decision ledger all agree.

## What Is Explicitly Out Of Scope Right Now

Do not spend active experiment bandwidth on:

- expanding obligation families
- promoting bounded LLM adjudication
- broad default-on rollout arguments outside the fixed repo set
- introducing additional benchmark repos

Those will reopen only after the default lane and `large_file` questions are resolved.
