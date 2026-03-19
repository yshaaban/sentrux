# Parallel-Code Proof Snapshot

Generated from: `<sentrux-root>/docs/v2/examples/parallel-code-golden`
Benchmark: `<sentrux-root>/docs/v2/examples/parallel-code-benchmark.json`

## Top Findings

- `high` `closed_domain_exhaustiveness` (ConnectionBannerState) Closed domain 'ConnectionBannerState' is missing coverage for variants: connecting, reconnecting, restoring
- `high` `exact_clone_group` 2 functions share an identical normalized body across recently changed files
- `high` `exact_clone_group` 2 functions share an identical normalized body across recently changed files
- `high` `exact_clone_group` 2 functions share an identical normalized body across recently changed files
- `high` `exact_clone_group` 2 functions share an identical normalized body across recently changed files
- `medium` `closed_domain_exhaustiveness` (task_presentation_status) Closed domain 'task_presentation_status' is missing required update sites
- `medium` `exact_clone_group` 2 functions share an identical normalized body across recently changed files
- `medium` `exact_clone_group` 2 functions share an identical normalized body across recently changed files
- `medium` `exact_clone_group` 2 functions share an identical normalized body across recently changed files
- `low` `exact_clone_group` 4 functions share an identical normalized body across recently changed files

## Concept Summaries

- `ConnectionBannerState` score 3100: Concept 'ConnectionBannerState' has repeated high-severity ownership or access issues
- `task_presentation_status` score 1680: Concept 'task_presentation_status' has 1 missing update sites to complete

## Optimization Priorities

- `ConnectionBannerState` score 4700: Deduplicate concept 'ConnectionBannerState' after aligning the repeated clone surfaces around it

## Proof Targets

1. Ownership/boundary: `n/a`
2. Propagation/obligations: `task_presentation_status`
3. Duplication/hotspot: clone clone-family-0x7e50d49dc16ef925 / hotspot server/browser-control-plane.ts

## Benchmark Baseline

- cold process total: 16772.8 ms
- warm cached total: 888.7 ms
- warm patch-safety total: 4149.9 ms
