# Metrics And Signals

Sentrux exposes several different metric layers. They are not interchangeable.

- legacy structural health metrics answer "how healthy is this snapshot overall?"
- v2 patch-safety signals answer "what changed, what is risky, and what did this patch miss?"
- benchmark metrics answer "is the product fast enough and regressing?"

This page is the reference for what those fields mean and how to use them.

## Start Here As A User

If you are using Sentrux during code review or while iterating on a patch, read the output in this order:

1. Check `scan_trust` and `confidence` first.
2. Check `touched_concept_gate.decision` next. The decision can be `pass`, `warn`, or `fail`.
3. Read `findings`.
4. Read `obligations` when the patch touches shared concepts, parallel flows, or state-heavy code.
5. Use `debt_signals` and `watchpoints` to decide what to fix now versus inspect next.
6. Use `signal_delta` and `quality_signal` for whole-repo context.

The rest of this page is the deeper field-by-field reference.

## How To Read The Numbers

- Fields ending in `_0_10000` use a 0-10,000 scale where `10000` is best or most complete.
- Ratios such as `quality_signal`, `coupling_score`, `coverage_ratio`, or `evolution_score` are usually `0.0-1.0`.
- Counts are plain counts. They are useful for scope, not just ranking.
- Some outputs are not scores at all. `findings`, `obligations`, `clone_families`, `debt_signals`, and `watchpoints` are evidence-bearing review surfaces.

## Where The Metrics Show Up

| Surface | Main purpose |
|---|---|
| `sentrux check` / MCP `health` | Whole-repo structural context |
| `sentrux gate` | Patch-vs-baseline regression detection |
| MCP `check` / `findings` / `brief` | Primary v2 patch-safety and review output |
| MCP `obligations` | Required update-site and exhaustiveness analysis |
| MCP `parity` | Contract parity across related surfaces |
| MCP state-integrity output | Conservative state-model validation |
| benchmark artifacts in `docs/v2/examples/` | Release-readiness and latency regression tracking |

## Legacy Structural Health

The legacy structural model is still useful for repo-level context and baseline comparisons. It is the main source for `quality_signal` and the root-cause breakdown.

### Top-Level Structural Score

| Metric | Meaning | Usefulness |
|---|---|---|
| `quality_signal` | Geometric mean of five root-cause scores: `modularity`, `acyclicity`, `depth`, `equality`, `redundancy`. Higher is better. | Fast whole-repo signal for "is this snapshot structurally healthier or worse?" |
| `bottleneck` | The weakest root-cause dimension in the current snapshot. | Good for deciding what category of cleanup matters most. |

### Root-Cause Scores

| Metric | Meaning | Usefulness |
|---|---|---|
| `modularity` | How well dependencies cluster into real modules instead of bleeding everywhere. | Useful for architecture-boundary health. |
| `acyclicity` | Whether circular dependencies exist and how much cycle pressure is present. | Useful for maintainability and change safety. |
| `depth` | Longest dependency chain in the repo. | Useful for layered-architecture sanity and blast-radius intuition. |
| `equality` | How evenly complexity is distributed rather than piling into a few files/functions. | Useful for god-object and concentration risk. |
| `redundancy` | How much dead or duplicated code exists. | Useful for cleanup, drift prevention, and maintenance cost control. |

### Dependency And Structure Diagnostics

| Metric | Meaning | Usefulness |
|---|---|---|
| `coupling_score` | Harmful cross-module coupling, especially to unstable targets. Lower is better. | Useful for explaining why a patch widened structural blast radius. |
| `total_import_edges` | Total import edges in the analyzed graph. | Scope/context diagnostic. |
| `cross_module_edges` | Import edges crossing module boundaries. | Useful for understanding how much traffic leaves local modules. |
| `circular_dep_count` | Number of cycle clusters. | Quick tangle indicator. |
| `circular_dep_files` | Files participating in each cycle cluster. | Useful for actionable untangling work. |
| `entropy` | Normalized Shannon entropy of cross-module edge distribution. High means cross-module traffic is spread broadly instead of following a few clear seams. | Useful as a structural "noise" diagnostic. |
| `avg_cohesion` | Average intra-module cohesion. | Useful for distinguishing coherent modules from grab-bag directories. |
| `max_depth` | Longest dependency chain. | Useful for identifying brittle layering. |

### File And Function Outliers

| Metric | Meaning | Usefulness |
|---|---|---|
| `god_files` | Files with fan-out above threshold. | Useful for broad orchestrators that know too much. |
| `hotspot_files` | Files with high fan-in that are also unstable. | Useful for high-risk coordination choke points. |
| `most_unstable` | Highest Martin instability files, with `fan_in` and `fan_out`. | Useful for spotting files that depend outward a lot but are not stable foundations. |
| `complex_functions` | High cyclomatic complexity functions. | Useful for local refactor targets. |
| `cog_complex_functions` | High cognitive-complexity functions. | Useful for readability and reviewability. |
| `long_functions` | Overlong functions. | Useful for extraction opportunities. |
| `high_param_functions` | Functions with too many parameters. | Useful for interface simplification. |
| `long_files` | Oversized files. | Useful for sprawl reduction. |
| `duplicate_groups` | Exact duplicate function bodies. | Useful for clone cleanup and drift prevention. |
| `dead_functions` | Functions with no detected call sites. | Useful for safe cleanup candidates. |

### Ratios

| Metric | Meaning | Usefulness |
|---|---|---|
| `god_file_ratio` | God files / total files. | Useful for repo-level outlier density. |
| `hotspot_ratio` | Hotspot files / total files. | Useful for coordination concentration. |
| `complex_fn_ratio` | Complex functions / total functions. | Useful for overall code difficulty trend. |
| `long_fn_ratio` | Long functions / total functions. | Useful for readability trend. |
| `comment_ratio` | Comments / total lines. | Context-only, not a quality proxy on its own. |
| `large_file_ratio` | Large files / total files. | Useful for sprawl trend. |
| `duplication_ratio` | Duplicate functions / total functions. | Useful for clone pressure trend. |
| `dead_code_ratio` | Dead functions / total functions. | Useful for cleanup trend. |
| `high_param_ratio` | High-parameter functions / total functions. | Useful for API shape drift. |
| `cog_complex_ratio` | Cognitively complex functions / total functions. | Useful for review complexity trend. |

## Test-Gap Metrics

These are structural test-coverage signals, not runtime line coverage.

| Metric | Meaning | Usefulness |
|---|---|---|
| `source_files` | Non-test source file count. | Scope/context. |
| `test_files` | Detected test file count. | Scope/context. |
| `tested_source_files` | Source files imported by tests. | Coverage breadth signal. |
| `untested_source_files` | Source files with no detected test imports. | Missing-test inventory. |
| `coverage_ratio` / `coverage_score` | Tested source files / total source files. | Useful for repo-level test reach trend. |
| `gaps[].risk_score` | Untested file risk ranked as `max_complexity × (fan_in + 1)`. | Useful for prioritizing which missing tests actually matter. |
| `gaps[].max_complexity` | Maximum complexity in the untested file. | Shows how hard the file is to trust untested. |
| `gaps[].fan_in` | Number of importers of the untested file. | Shows how broadly a bug could spread. |

## Evolution Metrics

These use git history rather than just the current snapshot.

| Metric | Meaning | Usefulness |
|---|---|---|
| `churn[path]` | Per-file commit count and line churn over the lookback window. | Useful for finding frequently changing files. |
| `coupling_pairs` | File pairs that change together often. | Useful for hidden coordination boundaries. |
| `hotspots` | Temporal hotspots ranked as `churn_count × max_complexity`. | Useful for prioritizing risky refactors. |
| `code_age[path]` | Days since the file last changed. | Useful for distinguishing fresh churn from stable areas. |
| `authors[path]` | Author distribution for a file. | Useful for ownership concentration. |
| `single_author_ratio` | Fraction of files effectively owned by one author. | Bus-factor warning signal. |
| `bus_factor_score` | Higher is better. `1.0` means ownership is not highly concentrated. | Useful for continuity risk. |
| `churn_score` | Higher is better. `1.0` means churn is spread more uniformly instead of piling into a few files. | Useful for hotspot concentration risk. |
| `evolution_score` | Conservative history score, currently the minimum of `bus_factor_score` and `churn_score`. | Useful for overall historical fragility. |

## V2 Patch-Safety Signals

These are the main public-beta surfaces. They answer changed-scope review questions, not just repo-level health questions.

### Scan Trust And Confidence

| Metric | Meaning | Usefulness |
|---|---|---|
| `scan_trust.mode` | How the scan ran. | Context for interpreting the rest of the payload. |
| `scan_trust.scope_coverage_0_10000` | Fraction of candidate files kept in scope. | If low, the result is incomplete by construction. |
| `scan_trust.overall_confidence_0_10000` | Aggregate scan confidence after accounting for exclusions, fallback mode, partial scans, and dependency resolution. | Main trust dial for the current scan. |
| `scan_trust.resolution.internal_confidence_0_10000` | How well internal dependency resolution succeeded. | Useful when semantic/scope outputs look suspiciously thin. |
| `scan_trust.partial` / `scan_trust.truncated` | Whether the scan had to stop short or narrow scope. | Explains confidence penalties. |
| `confidence.scan_confidence_0_10000` | Summary confidence score derived from scope and resolution. | Quick answer to "should I trust this run?" |
| `confidence.rule_coverage_0_10000` | How much of the configured v2 rule surface is active. | Tells you whether the repo has a rich or thin rules model. |
| `confidence.semantic_rules_loaded` | Whether semantic rules are active. | If false, some higher-value findings will be absent. |
| `confidence.session_baseline` | Whether the v2 session baseline loaded and is schema/project compatible. | Important for trusting `gate` and `session_end` comparisons. |

### Findings And Clone Signals

| Metric | Meaning | Usefulness |
|---|---|---|
| `findings` | Main merged v2 findings for the review surface. | This is the primary "what is risky or incomplete?" answer. |
| `finding_details` | More review-ready detail for the surfaced findings. | Useful when you need evidence, not just a headline. |
| `semantic_finding_count` | Count of semantic findings currently visible. | Useful for separating semantic signal from structural signal volume. |
| `clone_group_count` | Count of exact duplicate groups seen in scope. | Raw duplication pressure. |
| `clone_family_count` | Count of higher-level clone families clustered from duplicate groups. | Better signal for real follow-through drift risk than raw duplicates alone. |
| `visible_clone_group_count` / `visible_clone_family_count` | Clone counts that survived filtering/ranking for the current payload. | Useful for understanding what the user actually sees. |
| `clone_families` | Family-level duplicate summaries with reasons and metrics. | Useful for deciding whether copies should stay synchronized or be collapsed. |
| `clone_remediations` | Remediation hints for visible clone families. | Useful for concrete next steps. |

### Obligations, Parity, And State Integrity

| Metric | Meaning | Usefulness |
|---|---|---|
| `obligations` | Required update sites implied by configured concepts or closed domains. | Useful for catching partial edits and forgotten propagation. |
| `obligation_count` | Number of obligations in scope. | Scope/context. |
| `missing_site_count` | Missing update sites across obligations. | Useful as direct incompleteness pressure. |
| `context_burden` | Aggregated context burden carried by the current obligation set. | Helps explain why a patch is hard to reason about. |
| `obligation_completeness_0_10000` | Completeness of required update sites. `10000` means no missing obligation sites. | Core patch-completeness metric. |
| `parity_score_0_10000` | Alignment score for configured related surfaces. | Useful for mirrored logic, parallel flows, or contract synchronization. |
| `state_integrity_score_0_10000` | Conservative score for configured state-model integrity. | Useful for lifecycle/state transition hardening. |

### Session And Gate Deltas

| Metric | Meaning | Usefulness |
|---|---|---|
| `introduced_findings` | Findings present now that were not present in the baseline. | Useful for patch regression review. |
| `resolved_findings` | Findings that disappeared relative to baseline. | Useful for proving improvement. |
| `introduced_clone_finding_count` | New clone findings introduced by the patch. | Useful for duplication regression tracking. |
| `changed_files` | Files in changed scope. | Basic review context. |
| `changed_concepts` | Concepts implicated by the changed scope. | Useful for understanding why an obligation or gate fired. |
| `touched_concept_gate.decision` | Pass/fail gate decision for changed-scope findings and obligations. | Main patch gate outcome. |
| `signal_before`, `signal_after`, `signal_delta` | Legacy structural signal before/after against the saved baseline. | Useful for whole-repo regression context even when the v2 gate is the primary decision. |
| `coupling_change`, `cycles_change` | Legacy coupling/cycle deltas against the saved baseline. | Useful as supporting structural context. |

### Debt Signals And Watchpoints

These summarize surfaced problems into review and remediation queues.

| Metric | Meaning | Usefulness |
|---|---|---|
| `debt_signal_count` | Number of trusted structural debt signals. | Useful for prioritized cleanup volume. |
| `debt_signals` | Trusted debt items synthesized from structural reports, findings, obligations, and clone pressure. | Good for prioritizing maintenance work with evidence. |
| `experimental_debt_signal_count` | Debt items intentionally quarantined from default top-line trust. | Useful for calibration and cautious inspection. |
| `watchpoint_count` | Lower-confidence or monitor-this signals. | Useful for review attention without over-claiming certainty. |
| `watchpoints` | Structured watchpoint payloads. | Good for "inspect this next" queues. |
| `score_0_10000` | Severity/ranking score for a debt signal or watchpoint. | Useful for ordering work. |
| `trust_tier` | `trusted`, `watchpoint`, or `experimental`. | Tells you how hard Sentrux is willing to lean on the signal. |
| `presentation_class` | How the signal should be framed, such as `structural_debt`, `hardening_note`, or `watchpoint`. | Useful for human-readable triage. |
| `signal_class` | Broad class such as `debt` or `watchpoint`. | Useful for queue separation. |
| `signal_families` | Families such as `ownership`, `clone`, `dependency`, `state`, or `boundary`. | Useful for thematic grouping. |

## Structural Debt Report Kinds

The structural analyzer currently synthesizes debt reports for patterns such as:

- large files and file sprawl
- dependency sprawl
- unstable hotspots
- cycle clusters and cut candidates
- dead private code clusters and dead islands
- coordination hotspots

Each report carries:

- summary and impact
- evidence and inspection focus
- `score_0_10000`
- trust and presentation classification
- structured metrics such as `fan_in`, `fan_out`, `instability_0_10000`, `line_count`, `function_count`, `cycle_size`, `max_complexity`, `dead_symbol_count`, `public_surface_count`, `guardrail_test_count`, and related fields when relevant

## Suppression And Experimental Metrics

These fields are important because absence is not always the same as "no problem."

| Metric | Meaning | Usefulness |
|---|---|---|
| `suppressed_finding_count` | Findings hidden by active suppressions. | Useful for understanding what was intentionally muted. |
| `suppression_hits` | Which suppressions matched. | Useful for suppression auditing. |
| `expired_suppression_match_count` | Suppressions that matched but are past expiry. | Useful for cleanup and policy enforcement. |
| `experimental_finding_count` | Findings held out of the default top-level review surface because they are still experimental. | Useful for calibration and careful inspection. |

## Benchmark Metrics

Benchmark artifacts under `docs/v2/examples/` track product latency, not repo quality.

The current benchmark suite records metrics such as:

- cold process startup
- cold scan
- cold concepts
- cold `agent_brief` onboarding
- warm cached total
- warm `findings`
- warm persisted total
- warm persisted concepts
- warm persisted `findings`
- warm patch-safety total
- warm `session_start`
- warm `agent_brief` patch
- warm `gate`
- warm `check`
- warm `agent_brief` pre-merge
- warm `session_end`

Use these to answer:

- did a release make the local agent loop slower?
- did persisted baseline reuse actually help?
- are the warm paths fast enough for interactive use?

Do not use benchmark metrics as code-health scores.

## Practical Reading Order

If you only need one fast path:

1. Start with `scan_trust` and `confidence`.
2. Read `findings`.
3. Read `obligations` if the patch touches concept-heavy or state-heavy code.
4. Use `debt_signals` and `watchpoints` for prioritization.
5. Use `quality_signal` and the legacy baseline delta for whole-repo context.

That order usually matches how much the numbers help during real code review.
