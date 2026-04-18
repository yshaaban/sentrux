# Sentrux V2 Experiment Program

Last audited: 2026-04-19

## Purpose

This document turns the current v2 gaps into a concrete experiment program.

The goal is not to try random tweaks until the metrics move. The goal is to explore a bounded set of plausible directions, measure them with the existing evaluation stack, and turn the results into explicit keep, demote, or expand decisions.

This program is specifically aimed at the three current product gaps:

- the default agent lane is more coherent than before, but it does not yet reliably improve repair behavior
- broad structural pressure, especially `large_file`, is still suspect as an intervention-grade signal
- obligation breadth and treatment-vs-baseline proof are not yet strong enough to close the v2 completion bar

This document should be read alongside:

- [Master Plan](./master-plan.md)
- [Completion Execution Tracker](./completion-execution-tracker.md)
- [Experiment Records](./experiments/README.md)
- [Testing And Validation](./testing-and-validation.md)
- [Validation Loop](./validation-loop.md)
- [Policy And Eval Architecture](./policy-and-eval-architecture.md)
- [Eval Harness](./evals/README.md)

## Execution Surfaces

The experiment program now has checked-in execution surfaces:

- machine-readable experiment specs under [./evals/experiments](./evals/experiments)
- the experiment schema at [./evals/experiment.schema.json](./evals/experiment.schema.json)
- the experiment runner at [../../scripts/evals/run-experiment.mjs](../../scripts/evals/run-experiment.mjs)
- the experiment tracker builder at [../../scripts/evals/build-experiment-tracker.mjs](../../scripts/evals/build-experiment-tracker.mjs)

That means the remaining workstreams are now executable and auditable as tracked experiments, not just narrative plan items.

## Current Honest Read

The current checked-in evidence says the following:

- offline ranking quality is strong enough to continue investing in the narrowed wedge
- the live intervention outcome is still not good enough to call the signals product-good
- `large_file` is currently a real demotion candidate, not just a low-priority cleanup item

Repository-backed evidence from the latest inspected Sentrux repo-local loop:

- `top_action_follow_rate = 0.333`
- `top_action_help_rate = 0`
- `task_success_rate = 0.667`
- `promotion_candidate_count = 0`
- `demotion_candidate_count = 1`
- `ranking_miss_count = 1`
- `top_action_failure_count = 4`

The consequence is simple:

- do not expand analyzer scope yet
- do not treat reviewed precision as sufficient proof
- do not keep broad structural signals in the default lane on intuition alone
- use targeted experiments to decide what stays in the intervention wedge

## Program Rules

Every experiment in this program must follow these rules.

1. Freeze the tested ranking and gating policy for the duration of the experiment cycle.
2. Change one causal dimension at a time.
3. Use the existing evaluation stack rather than ad hoc evidence.
4. Prefer replay and seeded tasks for cheap screening, then confirm on paired live tasks.
5. Promote or demote signals only from measured outcome changes.
6. Keep the agent lane and maintainer lane separate in both design and reporting.

## Primary Questions

This program is meant to answer five questions.

1. Which signal families actually improve agent outcomes when placed in the default lane?
2. Should `large_file` remain in the default lane at all, and if so under what exact restrictions?
3. Which missing-followthrough families materially improve repair behavior when obligation breadth is expanded?
4. Does treatment beat baseline on fixed tasks and fixed repos strongly enough to drive promotion decisions?
5. Does bounded LLM adjudication improve help rate enough to justify its cost and latency?

## Experiment Structure

Each workstream uses the same three-stage structure.

### Stage 1: Screen

Purpose:

- cheaply explore several directions
- eliminate weak variants early

Recommended scope:

- 4 to 8 policy variants or feature variants
- replay tasks, seeded defects, and public-safe fixture tasks
- no promotion decisions yet

Exit bar:

- identify the top 2 variants worth confirmation

### Stage 2: Confirm

Purpose:

- run paired treatment-vs-baseline tasks on fixed task sets
- verify that the apparent gain survives outside the screening loop

Recommended scope:

- 2 shortlisted variants
- at least 3 repos
- at least 12 fixed tasks per variant across those repos
- canonical experiment arms with `no_intervention` as baseline

Exit bar:

- one variant clearly outperforms the others on the primary outcome metrics

### Stage 3: Decide

Purpose:

- convert evidence into a product decision

Allowed decisions:

- promote
- keep experimental
- demote to maintainer lane
- discard
- expand with a follow-up experiment

Every decision must name:

- what was tested
- what won
- what lost
- what evidence was decisive
- what changed in product policy as a result

## Metrics That Decide Experiments

The default decision metrics for this program are:

- `top_action_follow_rate`
- `top_action_help_rate`
- `task_success_rate`
- `top_action_failure_count`
- `patch_expansion_rate`
- `intervention_net_value_score`
- `reviewer_disagreement_rate`
- `ranking_miss_count`
- `promotion_candidate_count`
- `demotion_candidate_count`

Secondary metrics:

- `reviewed_precision`
- `top_1_actionable_precision`
- `top_3_actionable_precision`
- `repair_packet_complete_rate`
- `repair_packet_fix_surface_clear_rate`
- `repair_packet_verification_clear_rate`
- `session_trial_miss_rate`
- `remediation_success_rate`

Primary metrics should decide promotion and demotion. Secondary metrics should explain why.

## Workstream A: Default-Lane Intervention Quality

### Goal

Make the default lane consistently small, causal, and worth following.

### Hypothesis

The right default lane is dominated by clone, propagation, boundary, and patch-local concentration signals. It becomes worse when broad structural pressure occupies lead slots without showing strong evidence of immediate repair leverage.

### Experiment A1: Default-Lane Family Ablation

Question:

- which signal families improve agent outcomes when included in the default lane

Variants to screen:

- current policy
- clone + obligation + rules only
- clone + obligation + rules + patch-local concentration
- current policy with all structural pressure removed from the default lane
- current policy with structural pressure restricted to patch-worsened and concrete-repair cases only

Evidence to compare:

- lead-surface composition
- `top_action_follow_rate`
- `top_action_help_rate`
- `task_success_rate`
- `ranking_miss_count`
- unexpected-top-action counts in evidence review

Likely outcome:

- use this experiment to decide which families remain eligible for lead slots

### Experiment A2: Slot Pressure And Compression

Question:

- what slot cap and watchpoint suppression level gives the best tradeoff between actionability and completeness

Variants to screen:

- 1 primary action
- 2 primary actions
- 3 primary actions
- 3 actions with stronger supporting-watchpoint suppression

Evaluation rule:

- do not change the public product cap until confirmation shows a clear win
- screening can simulate narrower caps from existing ranked outputs

Primary decision metric:

- `top_action_help_rate`

Secondary decision metrics:

- follow rate
- missed expected-signal count
- reviewer disagreement

### Experiment A3: Evidence-Aware Tie-Breakers

Question:

- which evidence tie-breakers actually improve lead selection

Variants to screen:

- trust tier and severity only
- current evidence-aware order
- evidence-aware order without treatment signals
- evidence-aware order without repair-packet quality
- evidence-aware order without patch-worsened preference

Primary decision metric:

- `top_action_help_rate`

Secondary decision metrics:

- `top_action_follow_rate`
- `ranking_miss_count`
- repair-packet usefulness in review verdicts

### Experiment A4: Repair-Packet Strictness

Question:

- how much repair-packet strictness is needed before a lead is genuinely actionable

Variants to screen:

- current repair-packet bar
- require clear fix surface only
- require fix surface plus verification
- require full complete packet

Primary decision metric:

- `top_action_help_rate`

Secondary decision metrics:

- follow rate
- intervention cost
- reviewer disagreement

### Exit Bar For Workstream A

This workstream is successful only when:

- the default lane remains within 1 to 3 primary actions on the tested repos
- the top action is materially more often helpful than in the current baseline
- broad watchpoint spillover is visibly reduced
- the selected policy stays stable across more than one repo shape

## Workstream B: Structural Pressure And `large_file`

### Goal

Determine whether broad structural pressure belongs in the default lane at all.

### Current Read

`large_file` is currently a demotion candidate and a ranking miss. That means the burden of proof has flipped: it should be treated as default-lane suspect until it earns its place.

### Hypothesis

Broad structural pressure is useful as maintainer context, but only rarely useful as a default intervention. If it remains in the default lane, it should do so only under narrow, evidence-backed conditions.

### Experiment B1: `large_file` Lane Removal

Question:

- does the default lane improve if `large_file` is removed entirely from lead selection

Variants:

- current policy
- `large_file` excluded from default lane
- all structural pressure excluded from default lane

Primary decision metric:

- overall default-lane help rate

Secondary decision metrics:

- ranking misses
- reviewer disagreement
- missed expected-signal counts for structurally motivated tasks

### Experiment B2: Strict Structural Re-entry

Question:

- if structural pressure is allowed back, what exact restrictions are needed

Variants:

- patch-worsened only
- patch-worsened plus clear fix surface
- patch-worsened plus clear fix surface plus evidence-backed helpfulness
- patch-worsened plus concrete cut candidate only

Primary decision metric:

- help rate when structural pressure is surfaced as the top action

Secondary decision metrics:

- task success
- intervention cost
- patch expansion

### Experiment B3: Structural Signal Splitting

Question:

- is `large_file` too broad to be useful as one signal kind

Directions to explore:

- owner-shell extraction pressure
- guarded-facade overload
- script or entry-surface accumulation
- patch-local concentration growth

The purpose is not to add more signals immediately. The purpose is to learn whether the useful part of `large_file` is actually a narrower, more causal intervention family.

### Exit Bar For Workstream B

Keep a structural signal in the default lane only if it:

- beats the no-structural variant on help rate
- does not increase ranking misses
- stays bounded on patch expansion
- remains fixable in review verdicts

If it does not clear that bar, move it to the maintainer lane and stop arguing with the data.

## Workstream C: Obligation Breadth And Precision

### Goal

Improve “what else must change?” so the tool can name the right sibling surfaces with enough specificity for an agent to act.

### Hypothesis

The next engine-quality gains come from broader obligation families and better changed-symbol precision, not from adding unrelated detector families.

### Target Obligation Families

- DTO and schema follow-through
- config keys and feature flags
- registries and lookup tables
- command and status surfaces
- public APIs and export surfaces
- sibling clone families
- tests, validation surfaces, and docs

### Experiment C1: Family Coverage Matrix

Question:

- which obligation families materially improve repair behavior when added

Method:

- build a fixed task matrix for each family with expected sibling surfaces
- use seeded patches plus public-safe real repo tasks
- record expected surfaces before running the analyzer

Decision metrics:

- expected-surface recall
- false obligation rate
- likely-fix-site hit rate
- verification-surface hit rate
- task-local precision

### Experiment C2: Changed-Symbol Precision

Question:

- can obligation expansion stay patch-local enough to avoid becoming noise

Variants:

- current symbol anchoring
- stricter declaration-only anchors
- declaration plus semantically related readers
- sibling-family expansion only when changed-scope evidence exists

Primary decision metric:

- false obligation rate

Secondary decision metrics:

- expected-surface recall
- reviewer disagreement

### Experiment C3: Repair Guidance Quality

Question:

- can the obligation engine name likely fix sites and verification surfaces clearly enough to raise help rate

Variants:

- diagnosis only
- diagnosis plus likely fix sites
- diagnosis plus likely fix sites plus verification surfaces

Primary decision metric:

- help rate on obligation-led tasks

Secondary decision metrics:

- remediation success
- patch expansion
- reviewer disagreement

### Negative Controls

Each obligation family experiment should include negative controls:

- unrelated file changes
- test-only changes
- nearby but non-owning surfaces

If a family cannot survive negative controls, it is not ready for promotion.

### Exit Bar For Workstream C

The obligation wedge is getting better only if it produces:

- higher expected-surface recall
- lower false obligation rate
- clearer likely fix sites
- better help rate on propagation-style tasks

## Workstream D: Treatment-Vs-Baseline Proof

### Goal

Make treatment-vs-baseline results the decisive promotion bar rather than supporting context.

### Hypothesis

The right intervention wedge will show positive outcome deltas on fixed tasks before it shows perfect-looking scorecards.

### Standard Experiment Arms

Use the canonical arms already supported by the current eval stack:

- `no_intervention`
- `report_only`
- `fix_this_first`
- `stop_and_refactor`

Do not add new arms casually. Only add a new arm when it represents a clearly different product behavior.

### Repo Set

Use:

- `sentrux`
- `parallel-code`
- `one-tool`
- at least one additional public-safe repo once it has a checked-in manifest and safe artifact path

### Task Set Rules

For each repo:

- freeze a task set for one experiment cycle
- balance across clone, propagation, boundary, and structural contexts
- keep prompts and expected outcomes stable

### What To Measure

At both repo level and signal level:

- follow delta versus baseline
- help delta versus baseline
- task success delta versus baseline
- patch expansion delta versus baseline
- intervention net value delta
- reviewer disagreement delta
- escaped regression count

### Required Decision Outputs

Each treatment cycle must answer:

- which arm won overall
- which signal families improved under treatment
- which signals still failed despite being surfaced
- whether any default-on promotion is justified

### Exit Bar For Workstream D

No signal should become default-on from local precision alone.

A signal is ready for promotion only if it shows:

- positive treatment evidence on fixed tasks
- stable help or success gains
- acceptable patch expansion
- acceptable reviewer disagreement
- repeated evidence across more than one repo

## Workstream E: Bounded LLM Adjudication

### Goal

Test whether a bounded adjudication loop improves outcomes after static and semantic narrowing.

### Gating Rule

Do not spend major effort here until Workstreams A through D produce a stable static baseline worth comparing against.

### Valid Use Cases

- reranking ambiguous candidates
- suppressing low-value findings that survive deterministic narrowing
- improving repair and verification guidance for already-supported findings

### Invalid Use Cases

- raw repo scanning
- ungrounded promotion
- free-form architectural critique as default behavior

### Experiment E1: Advisory Reranking

Compare:

- static-only ranking
- static-plus-adjudication ranking

Primary decision metric:

- help rate delta

Secondary decision metrics:

- latency
- cost
- reviewer disagreement

### Experiment E2: Guidance Enrichment

Compare:

- current repair packets
- current repair packets plus bounded adjudication guidance

Primary decision metric:

- help rate on followed actions

Secondary decision metrics:

- intervention cost
- patch expansion
- reviewer confidence

### Exit Bar For Workstream E

Keep bounded adjudication only if it:

- improves help rate materially
- does not inflate disagreement
- stays within acceptable latency and cost

If it does not clear that bar, keep it as offline analysis support only.

## Documentation Model

Every experiment should leave behind four kinds of documentation.

### 1. Experiment Brief

Path:

- `docs/v2/experiments/<date>-<slug>.md`

Contents:

- question
- hypothesis
- tested variants
- repo set
- task set
- primary metrics
- stop rule
- success bar

### 2. Machine-Readable Artifacts

Path:

- `.sentrux/evals/<timestamp>-<slug>/`

Required artifacts:

- session telemetry summary
- signal scorecard
- session corpus
- evidence review

Where possible, preserve `program_id` and `phase_id` through the existing manifests and downstream artifacts.

### 3. Decision Note

Path:

- append to the experiment brief or add `decision.md` beside it

Required contents:

- winner
- loser
- decisive metrics
- what changes next
- what is explicitly not changing yet

### 4. Program Tracking Updates

Update:

- [completion-execution-tracker.md](./completion-execution-tracker.md)
- [roadmap.md](./roadmap.md) when the decision changes implementation priorities

## Suggested First Three Cycles

### Cycle 1: Default-Lane And `large_file` Screening

Run:

- Workstream A1
- Workstream A2
- Workstream B1

Decision goal:

- identify the best default-lane family set
- decide whether `large_file` should be removed from the default lane immediately

### Cycle 2: Obligation Breadth

Run:

- Workstream C1
- Workstream C2
- Workstream C3

Decision goal:

- identify which obligation families deliver the best repair leverage per unit of complexity

### Cycle 3: Confirmed Treatment Program

Run:

- Workstream D on the winning default-lane policy and the best obligation variant

Decision goal:

- decide whether any signal family is actually ready for promotion or default-on movement

Only after Cycle 3 should bounded adjudication enter the main experiment queue.

## What Not To Do

- do not keep broadening analyzer coverage while help rate is weak
- do not keep shipping policy tweaks without frozen-cycle evaluation
- do not defend `large_file` as a default-lane signal unless it wins on outcomes
- do not promote signals from reviewed precision alone
- do not let LLM adjudication become a substitute for grounded evidence

## Completion Bar For This Program

This experiment program is working only when it produces all of the following:

- a stable default-lane signal set that repeatedly helps more than it harms
- a clear decision on whether `large_file` belongs in the default lane
- obligation expansion that improves propagation-task outcomes without exploding false positives
- treatment-vs-baseline proof that materially influences promotion and default-on decisions
- a documented, repeatable evidence trail for every major signal-family decision
