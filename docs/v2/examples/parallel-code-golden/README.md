# Parallel-Code Scoped Golden Outputs

These files are the first real-repo v2 golden outputs captured against the repo configured by `PARALLEL_CODE_ROOT` (the maintainer default is the sibling checkout `../parallel-code`).

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
- `agent-brief-onboarding.json`
- `agent-brief-patch.json`
- `agent-brief-pre-merge.json`
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
- mode-aware `agent_brief` outputs now capture repo onboarding, patch guidance, and pre-merge guidance from the same MCP scan state
- real-repo `session_start`, `gate`, and `session_end` pass outputs are now checked in using a temporary local clone rather than the live working tree
- real-repo regression-path `gate` and `session_end` fail outputs now exist using a deterministic mutation in `src/components/SidebarTaskRow.tsx`
- the current regression fixture is intentionally minimal: one injected forbidden raw read that should fail patch safety on a real `parallel-code` rule

Those are not reasons to hide the outputs. They are reasons to keep them versioned.

## Refresh Command

```bash
./scripts/refresh_parallel_code_goldens.sh
```

The refreshed metadata now records the source tree identity and freshness details:

- commit
- dirty path count and dirty path list
- tree fingerprint
- analysis mode

## Live Report Command

```bash
node scripts/generate_parallel_code_live_engineer_report.mjs
```

This command refuses stale goldens by default. Use `--allow-stale-goldens` only when you are intentionally inspecting an older captured baseline.

## Validation Command

Use the one-command validation loop to compare fresh outputs against the checked-in goldens:

```bash
node scripts/validate_parallel_code_v2.mjs
```

Use the benchmark-only path when you just want timing data:

```bash
node scripts/benchmark_parallel_code_v2.mjs
```

Optional environment overrides:

- `PARALLEL_CODE_ROOT=/path/to/parallel-code`
- `OUTPUT_DIR=/custom/output/dir`
- `SENTRUX_BIN=/path/to/sentrux`
- `ALLOW_STALE_GOLDENS=1` for the live report generator

## Stability Note

The refresh script now clones the source repo into a temporary local copy before running MCP requests.

That means:

- checked-in goldens do not depend on whatever uncommitted state happens to exist in the live `parallel-code` worktree
- absolute temp-copy paths are sanitized back to the source repo root before the JSON is written
- regression fail goldens are generated from a deterministic temporary mutation, not from a hand-edited live checkout
