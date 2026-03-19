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

## Debt Signals

- `clone_family` `clone-family-0x7e50d49dc16ef925` score 86: 4 exact clone groups repeat across 2 files and churn differs by 0 recent commit(s) across siblings; sibling file age spans 1 day(s)
- `clone_family` `clone-family-0x9ebb8dad5cafb9c0` score 78: 4 exact clone groups repeat across 2 files and churn differs by 3 recent commit(s) across siblings; sibling file age spans 0 day(s)
- `hotspot` `server/browser-control-plane.ts` score 5350: File 'server/browser-control-plane.ts' is a coordination hotspot worth refactoring before adding more behavior
- `hotspot` `electron/ipc/hydra-adapter.ts` score 4827: File 'electron/ipc/hydra-adapter.ts' is a coordination hotspot worth refactoring before adding more behavior
- `concept` `ConnectionBannerState` score 4305: Concept 'ConnectionBannerState' has repeated high-severity ownership or access issues

## Watchpoints

- `ConnectionBannerState` score 4700: Deduplicate concept 'ConnectionBannerState' after aligning the repeated clone surfaces around it

## Proof Targets

1. Ownership/boundary: `n/a`
2. Propagation/obligations: `task_presentation_status`
3. Duplication/hotspot: clone clone-family-0x7e50d49dc16ef925 / hotspot server/browser-control-plane.ts

## Benchmark Baseline

- cold process total: 16772.8 ms
- warm cached total: 888.7 ms
- warm patch-safety total: 4149.9 ms
