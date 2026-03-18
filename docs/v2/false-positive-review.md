# Sentrux V2 False-Positive Review

## Purpose

V2 only becomes useful in CI if teams trust the findings enough to ratchet them.

This document defines the minimum review workflow for any analyzer or heuristic change that can:

- show up in `findings`
- affect `gate`
- affect `session_end`
- be used in CI ratchets

## When To Run This Review

Run a false-positive review when any of these change:

1. a new analyzer is added
2. an analyzer starts emitting a new finding kind
3. heuristics or thresholds change for an existing analyzer
4. ranking changes alter which findings surface in top lists
5. suppression scope changes alter what is visible by default

## Review Inputs

Every review should include:

1. the exact commit under review
2. the benchmark repos used
3. the commands used to generate findings
4. the top findings or affected findings under review
5. the reviewer classification for each sampled finding

Minimum repo set for wedge analyzers:

1. `parallel-code`
2. one small synthetic fixture repo
3. one second TS repo when available

## Sampling Rules

Review at least:

1. top 10 findings for each affected analyzer on `parallel-code`
2. all findings triggered by the synthetic fixture that was meant to exercise the change
3. at least 5 borderline findings if the analyzer is heuristic-heavy

If the analyzer produces fewer than 10 findings, review all of them.

## Classification

Each reviewed finding must be classified as one of:

1. `true_positive`
   The finding is correct and useful.

2. `acceptable_warning`
   The finding is technically noisy or low-priority, but still acceptable to surface.

3. `false_positive`
   The finding is wrong or misleading enough that it should not surface in its current form.

4. `inconclusive`
   The reviewer could not determine correctness quickly enough from source alone.

`inconclusive` should be treated as a trust problem, not a pass.

## Review Record Template

Use a table like this in the PR description or a linked review note.

| Repo | Finding Kind | Severity | Files | Classification | Notes | Action |
| --- | --- | --- | --- | --- | --- | --- |
| `parallel-code` | `forbidden_writer` | `high` | `src/store/git-status-polling.ts` | `true_positive` | Matches explicit rule and real authority drift | keep |
| `parallel-code` | `closed_domain_exhaustiveness` | `high` | `src/App.tsx` | `acceptable_warning` | Real gap, but evidence should be deduped | refine evidence |
| `parallel-code` | `exact_clone_group` | `high` | `src/App.tsx`, `src/remote/App.tsx` | `inconclusive` | Needs family-level grouping to judge usefulness | follow-up |

## Promotion Thresholds

For a new analyzer or major heuristic change:

1. no reviewed `high`-severity false positives in the sampled set
2. no more than 20% `false_positive + inconclusive` in the sampled set
3. any `acceptable_warning` that dominates the top findings list must have a documented follow-up

If the change affects `gate`, use a stricter bar:

1. zero reviewed blocking false positives
2. zero reviewed blocking inconclusive findings

## Required Follow-Up For Failures

If the review fails the thresholds above, do one of:

1. narrow the analyzer scope
2. lower severity
3. make the finding opt-in or rule-driven
4. improve evidence or dedupe
5. add a suppression path if the finding is valid but adoption-hostile

Do not promote the finding into gating behavior until the review passes.

## Current Review Priorities

The current highest-priority review targets are:

1. clone-family prioritization after the git-aware clone drift work
2. real-repo `session_end` and `gate` outputs once the checked-in goldens exist
3. any future expansion of state-integrity heuristics

## Exit Condition

The review is complete when:

1. the sampled findings are classified
2. any false positives have an action
3. the PR or follow-up issue links the review artifact
4. gating promotion decision is explicit
