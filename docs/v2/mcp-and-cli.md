# Sentrux V2 MCP And CLI Spec

## Design Goal

V2 should be consumable by agents without breaking the current MCP workflow.

That means:

- keep existing tools working
- make MCP `check` the primary fast-path patch surface
- add a mode-aware `agent_brief` as the synthesized guidance surface
- add patch-safety-first v2 tools
- make MCP `check` the primary guided entrypoint and `session_end` the primary confirmation touchpoint
- return objective findings, debt signals, watchpoints, and patch risks before score summaries

Current shipped distinction:

- MCP `check` is the v2 fast-path tool
- CLI `sentrux brief` and `sentrux gate` are the public v2 CLI entry points
- CLI `sentrux check` still names the legacy structural rules check

## Guidance Surface

MCP `check` is the primary structured guidance surface for agents in the coding loop.

It should return a ranked list of actions for the current patch in one fast call.

The calibration loop should treat the top-ranked `actions` from `check` as the primary experimental surface. Real session telemetry should record which action was shown first, whether the next `check` cleared it, and whether follow-up edits introduced new regressions.

`agent_brief` packages the same evidence into a mode-aware brief when the agent needs more context instead of forcing the agent to reconstruct workflow from raw tool output.

Modes:

- `repo_onboarding`: explain repo shape, critical concepts, rules, exclusions, and where to start
- `patch`: summarize the current change, findings, obligations, and touched-concept risk
- `pre_merge`: summarize remaining blockers, unresolved obligations, and merge readiness

MCP `check` is the entry point for the fastest loop. `agent_brief`, `session_end`, `findings`, `obligations`, `gate`, and `scorecard` remain the broader structured evidence.

## Product Surface Priority

For v2 integrations, the preferred order is:

1. `check`
2. `agent_brief`
3. `session_end`
4. `findings`
5. `obligations`
6. `gate`
7. `scorecard`
8. concept and parity inspection tools

This ordering should shape MCP design first and CLI convergence second. Any ranking or optimization-like output is a sorting aid, not the final decision.

Finding trust model:

- `trusted`: solid enough for normal engineer-facing findings and touched-concept ratchets
- `watchpoint`: real structural pressure or incomplete interpretation; useful for inspection, not for overconfident automation
- `experimental`: detector is still under evaluation and must not quietly influence top-level findings or CI decisions

Engineering leverage model:

- `architecture_signal`: repo-wide ownership or layering pressure with broad consequences
- `local_refactor_target`: a contained seam that is ready for tightening
- `boundary_discipline`: a facade or boundary where glue is at risk of absorbing policy logic
- `regrowth_watchpoint`: an intentionally central surface that should not broaden further
- `secondary_cleanup`: real cleanup worth tracking, but not the highest-leverage improvement target
- `hardening_note`: narrow completeness or exhaustiveness follow-up
- `tooling_debt`: maintenance burden on scripts and tooling surfaces

## Existing Tools To Preserve

Keep these unchanged during the initial v2 rollout:

- `scan`
- `rescan`
- `health`
- `session_start`
- `session_end`
- `check_rules`
- `evolution`
- `dsm`
- `test_gaps`

## Relationship Between `health` And V2 Tools

`health` remains the v1-compatible summary tool.

For v2:

- `health` should keep its legacy role as a lightweight structural summary
- `health` and CLI `check` should explicitly present themselves as legacy structural context, not as the primary v2 quality narrative
- the desktop metrics panel and export flow should also present structural scores as supporting context
- `findings`, `obligations`, `session_end`, and `gate` should carry the actionable v2 diagnostics
- any `quality_opportunities` or `optimization_priorities` fields should be treated as evidence-backed inspection candidates rather than engineer-owned prioritization
- `health` may eventually link to the presence of v2 data, but it should not duplicate full v2 finding lists

Agents should use v2 tools for patch-safety decisions.

## New Primary Tools

## `check`

Purpose:

- return fast changed-scope issues for the current patch as a flat, ranked action list

Arguments:

- none

Returns:

- gate
- summary
- changed files
- ranked `actions`
- flat `issues`
- diagnostics and availability

`check` is the default tool for mid-loop agent feedback. It should stay fast-path only and never fall back to all-scope expensive analysis.

Current CLI note:

- CLI `sentrux check` is not this surface yet
- CLI currently exposes v2 primarily through `sentrux brief` and `sentrux gate`

## `agent_brief`

Purpose:

- return the mode-aware structured guidance brief for the current repo, patch, or merge state

Arguments:

- `mode = "repo_onboarding" | "patch" | "pre_merge"`
- `limit`
- `strict` for `pre_merge`

Returns:

- mode-specific summary
- ranked `actions`
- prioritized guidance
- linked findings and obligations
- touched-concept risk and gate readiness
- relevant concepts, trust tiers, and leverage classes
- confidence

CLI parity:

- `sentrux brief --mode repo-onboarding [path]`
- `sentrux brief --mode patch [path]`
- `sentrux brief --mode pre-merge --strict [path]`

The CLI command should print the same structured JSON contract rather than inventing a separate prose summary.

## `findings`

Purpose:

- return concrete patch-safety findings, debt signals, watchpoints, and patch risks with evidence

Arguments:

- `severity`
- `kind`
- `concept`
- `scope = "all" | "changed"`
- `limit`

Returns:

- findings
- experimental findings
- evidence
- likely fix sites
- concept summaries for repeated concept pressure
- debt signals, debt clusters, and watchpoints
- trust tiers, leverage classes, impact, candidate split axes, and related surfaces
- quality-improvement opportunities as inspection candidates
- optimization candidates as legacy watchpoint aliases only
- top-level confidence summary

## `obligations`

Purpose:

- return required update sites and what is still missing

Arguments:

- `scope = "all" | "changed"`
- `concept`
- `file`
- `symbol`

Returns:

- obligation summaries
- missing sites
- satisfied sites
- obligation count
- derived context burden

## `gate`

Purpose:

- answer whether the current patch regressed touched concepts enough to warn or fail

Arguments:

- `scope = "changed"`
- `strict = true | false`

Returns:

- decision
- blocking findings
- confidence
- explanation

## `session_end` Upgrade

The upgraded `session_end` response is the primary confirmation surface after `check`.

It should add:

- ranked `actions`
- changed files
- changed concepts
- introduced findings
- introduced clone findings
- experimental findings
- resolved findings
- missing obligations
- concept summaries for changed concepts
- patch-scoped trusted debt signals, watchpoints, and experimental side-channel findings
- patch-scoped quality-improvement opportunities as inspection candidates
- patch-scoped optimization candidates as legacy watchpoint aliases
- track deltas
- touched-concept regression verdict
- confidence delta if coverage changed

## Secondary Tools

## `scorecard`

Purpose:

- return grouped v2 tracks and signal-quality status as supporting context

Arguments:

- none

Returns:

- core tracks
- context tracks
- future tracks if available
- confidence
- exclusions

## `concepts`

Purpose:

- list known concepts and their ownership or canonical adapters

Arguments:

- `kind`
- `limit`

## `explain_concept`

Purpose:

- drill into one concept

Arguments:

- `id`

Returns:

- anchors
- authoritative writers
- canonical accessors
- contracts
- findings
- obligations

## `trace_symbol`

Purpose:

- trace a symbol to concepts, readers, writers, and obligations

Arguments:

- `symbol`

## `parity`

Purpose:

- return contract parity analysis as supporting context

Arguments:

- `contract`
- `scope`

## Proposed Response Shapes

## `session_end`

```json
{
  "changed_files": [
    "src/app/task-workflows.ts"
  ],
  "changed_concepts": [
    "task_git_status"
  ],
  "concept_summaries": [
    {
      "concept_id": "task_git_status",
      "summary": "Concept 'task_git_status' shows 2 boundary/ownership findings and 2 missing update sites",
      "score_0_10000": 7800
    }
  ],
  "debt_signals": [
    {
      "kind": "concept",
      "scope": "task_git_status",
      "signal_class": "debt",
      "signal_families": [
        "ownership",
        "boundary",
        "propagation"
      ],
      "severity": "high",
      "summary": "Concept 'task_git_status' shows 2 boundary/ownership findings and 2 missing update sites",
      "inspection_focus": [
        "inspect write ownership and boundary enforcement",
        "inspect the explicit propagation sites and completeness tests for this concept"
      ]
    }
  ],
  "debt_clusters": [
    {
      "scope": "cluster:src/app/task-workflows.ts|src/store/core.ts",
      "severity": "high",
      "summary": "Files src/app/task-workflows.ts and src/store/core.ts intersect 3 debt signals: concept, dependency_sprawl, cycle_cluster",
      "signal_kinds": [
        "concept",
        "dependency_sprawl",
        "cycle_cluster"
      ]
    }
  ],
  "watchpoints": [
    {
      "scope": "task_git_status",
      "severity": "high",
      "summary": "Concept 'task_git_status' intersects boundary pressure, propagation burden",
      "inspection_focus": [
        "inspect write ownership and boundary enforcement",
        "inspect whether boundary erosion is making the propagation chain easier to miss"
      ]
    }
  ],
  "introduced_findings": [
    {
      "id": "multi_writer:task_git_status",
      "kind": "multi_writer_concept",
      "severity": "high",
      "summary": "taskGitStatus has more than one durable write path",
      "confidence": 9300
    }
  ],
  "finding_details": [
    {
      "kind": "multi_writer_concept",
      "scope": "task_git_status",
      "severity": "high",
      "summary": "taskGitStatus has more than one durable write path",
      "impact": "Multiple write paths make the concept easier to update inconsistently and harder to debug.",
      "inspection_focus": [
        "inspect which module should own writes for this concept"
      ]
    }
  ],
  "missing_obligations": [
    {
      "concept": "task_git_status",
      "kind": "canonical_projection_update",
      "site": "src/app/task-presentation-status.ts"
    }
  ],
  "touched_concept_gate": {
    "decision": "warn",
    "reason": "high-confidence regression on touched concept"
  }
}
```

## `findings`

```json
{
  "confidence": {
    "analysis_coverage_0_10000": 9200
  },
  "concept_summaries": [
    {
      "concept_id": "task_git_status",
      "dominant_kinds": [
        "multi_writer_concept",
        "forbidden_writer"
      ],
      "summary": "Concept 'task_git_status' has 2 high-severity ownership or access findings"
    }
  ],
  "debt_signals": [
    {
      "kind": "concept",
      "scope": "task_git_status",
      "signal_class": "debt",
      "signal_families": [
        "ownership",
        "boundary"
      ],
      "severity": "high",
      "summary": "Concept 'task_git_status' has 2 high-severity ownership or access findings"
    }
  ],
  "debt_clusters": [
    {
      "scope": "cluster:src/app/task-workflows.ts|src/store/core.ts",
      "severity": "high",
      "summary": "Files src/app/task-workflows.ts and src/store/core.ts intersect 3 debt signals: concept, dependency_sprawl, cycle_cluster"
    }
  ],
  "watchpoints": [
    {
      "scope": "task_git_status",
      "severity": "high",
      "summary": "Concept 'task_git_status' intersects boundary pressure and clone overlap"
    }
  ],
  "finding_details": [
    {
      "kind": "public_api_bypass",
      "scope": "tasks",
      "severity": "high",
      "summary": "task-workflows imports store internals directly instead of the intended public store boundary",
      "impact": "Bypassing the intended read boundary weakens architectural contracts and can create stale or inconsistent views.",
      "inspection_focus": [
        "inspect whether reads should move behind the canonical accessor or public boundary"
      ]
    }
  ],
  "findings": [
    {
      "id": "public_api_bypass:task_workflows",
      "kind": "public_api_bypass",
      "severity": "high",
      "concept": "tasks",
      "summary": "task-workflows imports store internals directly instead of the intended public store boundary",
      "evidence": [
        {
          "file": "src/app/task-workflows.ts",
          "detail": "imports setStore from ../store/core"
        }
      ],
      "confidence": 9100,
      "likely_fix_sites": [
        {
          "site": "src/store/store.ts"
        }
      ]
    }
  ]
}
```

## `obligations`

```json
{
  "concept": "server_state_bootstrap",
  "missing": [
    {
      "kind": "registry_mapping",
      "site": "src/app/server-state-bootstrap-registry.ts"
    },
    {
      "kind": "browser_listener",
      "site": "src/runtime/browser-session.ts"
    }
  ],
  "satisfied": [
    {
      "kind": "category_list",
      "site": "src/domain/server-state-bootstrap.ts"
    }
  ],
  "context_burden": {
    "required_sites": 5,
    "affected_concepts": 1,
    "boundaries": 2
  }
}
```

## `scorecard`

```json
{
  "core_tracks": [
    {
      "id": "obligation_completeness",
      "score": 8600,
      "numerator": 43,
      "denominator": 50,
      "confidence": 9300
    }
  ],
  "context_tracks": [
    {
      "id": "analysis_coverage",
      "score": 8800,
      "numerator": 440,
      "denominator": 500,
      "confidence": 10000
    }
  ]
}
```

## CLI Surface

The CLI should mirror MCP priorities.

Suggested commands:

- `sentrux v2 findings`
- `sentrux v2 obligations`
- `sentrux v2 session-end`
- `sentrux v2 gate`
- `sentrux v2 scorecard`
- `sentrux v2 concepts`
- `sentrux v2 parity`

## MCP State Integration

Current MCP state:

- root
- cached snapshot
- cached health
- cached arch
- baseline

Add:

- `cached_v2`
- `cached_v2_baseline`

Do not overload the existing v1 baseline types.

Baseline coexistence and migration rules are defined in `baseline-migration.md`.

## Rollout Strategy

1. improve current `health` and baseline reporting first
2. ship `findings` and `obligations` before broader score tooling
3. upgrade `session_end` as soon as obligation analysis is usable
4. add `gate` when touched-concept regressions are trustworthy
5. add richer inspection tools after the wedge is stable

## Implementation Tasks

- [x] add `cached_v2`-style MCP state and patch-safety caches
- [ ] keep dedicated v2 handler extraction optional; current handlers carry v2 successfully
- [x] add `findings` tool
- [x] add `obligations` tool
- [x] upgrade `session_end` for v2 delta, gate, quality opportunities, and inspection candidates
- [x] add `gate` tool
- [ ] add `scorecard` tool
- [x] add `concepts` tool
- [x] add `explain_concept` tool
- [x] add `trace_symbol` tool
- [x] add `parity` tool
- [x] add CLI wrappers for the core v2 patch-safety flow
- [ ] finish GUI and remaining legacy-surface alignment
