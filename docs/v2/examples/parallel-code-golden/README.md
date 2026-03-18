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
- `session-start.json`
- `gate-pass.json`
- `gate-fail.json`
- `session-end-pass.json`
- `session-end-fail.json`
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
- real-repo `session_start`, `gate`, and `session_end` pass outputs are now checked in using a temporary local clone rather than the live working tree
- real-repo regression-path `gate` and `session_end` fail outputs now exist using a deterministic mutation in `src/components/SidebarTaskRow.tsx`
- the current regression fixture is intentionally minimal: one injected forbidden raw read that should fail patch safety on a real `parallel-code` rule

Those are not reasons to hide the outputs. They are reasons to keep them versioned.

## Refresh Command

```bash
./scripts/refresh_parallel_code_goldens.sh
```

Optional environment overrides:

- `PARALLEL_CODE_ROOT=/path/to/parallel-code`
- `OUTPUT_DIR=/custom/output/dir`
- `SENTRUX_BIN=/path/to/sentrux`

## Stability Note

The refresh script now clones the source repo into a temporary local copy before running MCP requests.

That means:

- checked-in goldens do not depend on whatever uncommitted state happens to exist in the live `parallel-code` worktree
- absolute temp-copy paths are sanitized back to the source repo root before the JSON is written
- regression fail goldens are generated from a deterministic temporary mutation, not from a hand-edited live checkout
