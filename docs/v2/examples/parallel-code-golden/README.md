# Parallel-Code Scoped Golden Outputs

These files are the first real-repo v2 golden outputs captured against:

- `<parallel-code-root>`

They are intentionally **scoped** goldens, not the final full regression suite.

## Why These Goldens Exist

They lock in what the current v2 implementation actually reports on the main case-study repo.

That matters for two reasons:

1. they give us a stable baseline for future analyzer changes
2. they expose current false positives and blind spots honestly

## What Is Included

- `scan.json`
- `concepts.json`
- `findings-top12.json`
- `explain-task_git_status.json`
- `explain-task_presentation_status.json`
- `explain-server_state_bootstrap.json`
- `obligations-task_presentation_status.json`
- `parity-server_state_bootstrap.json`
- `state.json`
- `metadata.json`

## Known Current Learnings

These goldens currently demonstrate several important v2 gaps:

- authority findings now ignore test-only writes, which makes the remaining `task_git_status` signal much more believable
- repeated same-file `forbidden_writer` evidence now collapses into a single top-level finding instead of duplicating identical entries
- projection concepts now use separate raw-read and write targets, so the example rules can keep upstream inputs without reintroducing multi-writer noise
- parity now recognizes the real bootstrap/runtime wiring for `server_state_bootstrap`
- explicit controller-style state models now map cleanly through discriminated unions and trailing `assertNever(...)` proofs
- zero-config exhaustiveness no longer lets giant transport domains like `IPC` dominate the top findings
- clone findings now have stable ids, git-aware churn/code-age context, deterministic instance ordering, and distinct-file recent-activity accounting
- clone findings still need family-level prioritization because related production clone groups can crowd the top list

Those are not reasons to hide the outputs. They are reasons to keep them versioned.

## Refresh Command

```bash
./scripts/refresh_parallel_code_goldens.sh
```

Optional environment overrides:

- `PARALLEL_CODE_ROOT=/path/to/parallel-code`
- `OUTPUT_DIR=/custom/output/dir`
- `SENTRUX_BIN=/path/to/sentrux`
