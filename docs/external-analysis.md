# External Repository Analysis

Use this workflow when you want to point Sentrux at a repo that is not the
Sentrux checkout and produce feedback for engineers who may not know anything
about Sentrux.

The goal is a practical engineering report, not a raw metric dump. A good run
should answer:

- what is safe to ignore right now
- what is worth fixing first
- where the likely fix sites are
- what validation should be rerun after the fix
- whether a later rerun shows real improvement

## Safety Model

`sentrux report` is the external-analysis entry point.

```bash
sentrux report /path/to/repo
```

By default it:

- analyzes an isolated workspace rather than writing into the target repo
- overlays working-tree changes from the target repo when the target is a Git repo
- writes artifacts outside the target repo
- avoids mutating the target repo
- does not upload repository contents
- does not call an LLM provider

`--mode live` is the exception. It scans the target path directly and analyzer
internals may write `.sentrux` state into that repo. Use the default isolated
mode for engineer-facing external analysis unless you specifically need live
local state.

## Prerequisites

- Sentrux binary or source checkout.
- Node.js 20+ when running from source, because the report command wraps
  `scripts/analyze-repo.mjs`.
- Read access to the target repository.
- Enough local disk space for an isolated analysis workspace and artifacts.

If the CLI cannot find the advisor script from a source checkout, set one of:

```bash
export SENTRUX_REPO_ROOT=/path/to/sentrux
export SENTRUX_ADVISOR_SCRIPT=/path/to/sentrux/scripts/analyze-repo.mjs
```

## Single-Repo Workflow

Use an explicit output directory so reports are easy to find and compare.

```bash
mkdir -p /tmp/sentrux-analysis/my-repo
sentrux report /path/to/my-repo \
  --repo-label my-repo \
  --output-dir /tmp/sentrux-analysis/my-repo \
  --findings-limit 100 \
  --dead-private-limit 50
```

Read the artifacts in this order:

1. `ADVISOR_SUMMARY.md`: short triage summary and artifact map.
2. `ENGINEERING_REPORT.md`: the report intended for the target repo's engineers.
3. `REPORT.md`: validation-oriented notes for Sentrux maintainers.
4. `scan-coverage-breakdown.md`: scan confidence, exclusions, and scope caveats.
5. `raw-tool-analysis.json` and `advisor-evidence.json`: machine-readable evidence
   when you need to debug ranking or write a custom summary.

The generated `<REPO>.suggested.rules.toml` is advisory. Do not check it into
the target repo until a maintainer of that repo agrees it matches the intended
architecture.

## Optional Calibrated Rerun

For a stronger external report, run a first pass, inspect the suggested rules,
then rerun with those rules as an explicit source. This still does not mutate the
target repo in the default isolated mode.

```bash
sentrux report /path/to/my-repo \
  --repo-label my-repo \
  --output-dir /tmp/sentrux-analysis/my-repo-calibrated \
  --rules-source /tmp/sentrux-analysis/my-repo/MY_REPO.suggested.rules.toml \
  --findings-limit 100 \
  --dead-private-limit 50
```

Use the calibrated report as the primary handoff when the suggested rules are
generic project-shape rules such as archetypes and exclusions. If the suggested
rules imply architecture boundaries that the repo owners have not confirmed,
leave them as a review topic instead of treating them as ground truth.

## Multi-Repo Projects

If one product is split across multiple repos, analyze each repo separately and
then synthesize one project-level report.

```bash
mkdir -p /tmp/sentrux-analysis/project

sentrux report /path/to/project-web \
  --repo-label project-web \
  --output-dir /tmp/sentrux-analysis/project/project-web \
  --findings-limit 100 \
  --dead-private-limit 50

sentrux report /path/to/project-worker \
  --repo-label project-worker \
  --output-dir /tmp/sentrux-analysis/project/project-worker \
  --findings-limit 100 \
  --dead-private-limit 50
```

For the synthesis:

- keep repo-specific findings attached to their repo
- call out cross-repo contract or schema drift only when there is evidence
- run each repo's own validation commands separately
- do not merge raw metrics into one score unless the repos share the same
  architecture, language mix, and validation profile
- summarize the product-level risks in plain engineering language

Repo labels are artifact labels only. They should not become hardcoded product
configuration in Sentrux.

## Pair With Target-Repo Validation

Sentrux does not automatically run arbitrary target-repo scripts because those
commands can install dependencies, call networks, mutate state, or take a long
time. After the report finishes, inspect the target repo's scripts and run the
safe local checks that repo already trusts.

Typical examples:

```bash
npm run typecheck
npm run lint
npm test
npm run architecture:check
npm run contracts:check
npm run cycles:check
```

When a project-specific check disagrees with Sentrux, report that distinction.
For example, if Sentrux flags broader mixed dependency pressure but the repo's
`madge` check passes, frame it as an architectural watchpoint rather than a hard
runtime import-cycle failure.

## How To Interpret The Report

Treat the findings in this order:

1. Required actions and missing obligations.
2. Concrete duplicate-drift findings with clear shared semantics.
3. Boundary or cycle seams where the first cut is narrow and testable.
4. Large-file and dependency-sprawl watchpoints.
5. Experimental stale-code or dead-private candidates.

Do not treat every structurally true issue as immediate work. The default lane is
intentionally small. Structural backlog is useful when it overlaps with current
feature work or explains repeated maintenance pain.

### Missing Obligations

Missing obligations are the highest-pressure findings because they usually mean
a changed concept was not propagated to all required surfaces. These are good
engineer-facing findings when they point to concrete files, tests, schema
surfaces, DTOs, routes, registries, or documentation that should change together.

### Duplicate Logic

Duplicate logic is actionable when the copies must stay behaviorally identical.
The safest first cut is usually a small shared helper or shared adapter module.

Do not centralize near-miss copies until you confirm the semantics are actually
the same. Similar helper names are not enough evidence.

### Cycles And Boundary Pressure

Cycle findings need careful wording. Distinguish:

- hard import cycles confirmed by the target repo's own tooling
- mixed dependency or interaction clusters that are maintainability pressure
- public-surface coupling where a facade or composition root is doing too much

The best repair is usually one seam, then a rerun. Avoid untangling an entire
cluster in one change.

### Large Files

Large-file findings are often real but should rarely lead the handoff by
themselves. Use them as guardrails:

- do not add unrelated behavior to the hotspot
- extract one cohesive responsibility when touching that area
- preserve tests around the extracted behavior
- do not split by line count alone

### Experimental Dead-Private Candidates

Dead-private analysis is intentionally cautious. Manual review is required before
deleting anything. Callback functions, event handlers, framework entry points,
JSX references, dynamically used symbols, and exported plugin hooks can look dead
to static analysis.

Engineer reports should say "review manually" for these candidates unless local
code inspection confirms they are stale.

## Rerun After Fixes

After engineers fix issues, rerun with the previous raw analysis to produce a
before/after comparison.

```bash
sentrux report /path/to/my-repo \
  --repo-label my-repo \
  --output-dir /tmp/sentrux-analysis/my-repo-after \
  --previous-analysis /tmp/sentrux-analysis/my-repo/raw-tool-analysis.json \
  --findings-limit 100 \
  --dead-private-limit 50
```

Use `BEFORE_AFTER.md` to answer:

- which primary actions were resolved
- whether new primary actions appeared
- whether clone, obligation, or large-file pressure changed
- whether scan confidence changed enough to affect interpretation

If fixes were made across multiple repos, rerun each repo and write one
project-level summary that separates per-repo outcomes from cross-repo outcomes.

## Engineer Handoff Format

A good handoff should be standalone. Avoid assuming the engineer knows Sentrux.

Recommended structure:

1. Scope and validation commands.
2. Executive summary.
3. Highest-ROI findings with file references and why they matter.
4. Smallest safe first cut for each finding.
5. What not to chase yet.
6. Verification checklist.

Use direct language:

- "This is a duplicate helper family; centralize only if semantics are identical."
- "This is a watchpoint, not a blocker."
- "The repo's own cycle check passed, so treat this as coupling pressure."
- "Do not delete this dead-private candidate without manual code review."

## Troubleshooting

If the output is too thin:

- check `scan-coverage-breakdown.md`
- inspect `confidence.scan_confidence_0_10000`
- confirm generated exclusions did not hide the source tree
- rerun with explicit `--findings-limit` and `--dead-private-limit`
- consider a calibrated rerun with reviewed suggested rules

If the output is too noisy:

- lead with `ADVISOR_SUMMARY.md`, not raw findings
- suppress experimental dead-private items from engineer-facing action lists
- demote large-file findings unless the current work touches that file
- verify duplicate findings manually before recommending extraction

If the target repo has no checked-in Sentrux rules:

- treat generated rules as a bootstrap suggestion
- use them inside isolated analysis first
- only check in `.sentrux/rules.toml` after repo maintainers confirm the intended
  boundaries

## Privacy Notes

External analysis is local by default. Reports can contain private file paths,
symbol names, and code-structure evidence from the target repo. Treat generated
artifacts as repository-sensitive unless the target repo is public and the report
has been reviewed for disclosure.
