# Repo Advisor Workflow

This workflow is the product-facing bridge from “given a repo” to useful feedback for an engineer who does not know Sentrux internals.

It does not replace the v2 analyzer, `agent_brief`, `gate`, or experiment program. It orchestrates those existing surfaces into a safe, repeatable single-repo analysis loop.

## Command

```bash
sentrux report /path/to/repo
```

The implementation is also available directly from a source checkout:

```bash
node scripts/analyze-repo.mjs --repo-root /path/to/repo
```

By default the command:

- creates an isolated analysis workspace
- overlays working-tree changes when the source repo is a Git repo
- writes artifacts under `~/.sentrux/repo-advisor/...`
- avoids mutating the target repo
- captures `scan`, `check`, `gate`, `findings`, `session_end`, and all three `agent_brief` modes
- generates a calibrated rules bootstrap from onboarding/project-shape evidence
- applies generated rules only inside the isolated analysis workspace when the workspace has no rules yet
- runs setup preflight checks for target mutation risk, workspace isolation, output paths, Node runtime, and rules-source safety
- emits engineer, validation, setup, rules, evidence, and optional before/after reports

Useful options:

```bash
sentrux report /path/to/repo --output-dir /tmp/report
sentrux report /path/to/repo --previous-analysis /tmp/old/raw-tool-analysis.json
sentrux report /path/to/repo --mode head
sentrux report /path/to/repo --mode live
sentrux report /path/to/repo --no-apply-suggested-rules
```

`live` mode still writes report artifacts outside the target repo unless `--output-dir` points into it, but it scans the target path directly and analyzer internals may write `.sentrux` state into that repo. It does not support `--rules-source` because applying alternate rules would require mutating the target repo. Generated rules are not auto-applied in `live` mode; rule auto-application is limited to isolated workspaces. Use the default isolated mode for engineer-facing external analysis.

The Rust CLI wrapper sets `SENTRUX_BIN` to the running binary before launching the Node-backed advisor. If `scripts/analyze-repo.mjs` cannot be located, run from a Sentrux source checkout, set `SENTRUX_REPO_ROOT` to the checkout root, or set `SENTRUX_ADVISOR_SCRIPT` directly to the advisor script path. The Node-backed workflow currently requires Node.js 20+.

## Artifact Contract

The workflow writes:

- `ADVISOR_SUMMARY.md`: short shareable summary of immediate patch actions, concrete follow-through surfaces, and artifact locations
- `ENGINEERING_REPORT.md`: engineer-facing report without requiring Sentrux vocabulary
- `REPORT.md`: validation-oriented report for Sentrux maintainers
- `SETUP_PREFLIGHT.md` and `setup-preflight.json`: safety/setup status, including whether the target repo can be mutated
- `RULES_BOOTSTRAP.md`: generated rules, risk labels, and evidence
- `<REPO>.suggested.rules.toml`: suggested rules for maintainer review
- `advisor-evidence.json`: machine-readable default-lane, large-file, obligation, and safety evidence
- `raw-tool-analysis.json`: raw MCP/tool payloads
- `raw-tool-summary.json`: summarized finding and scan-confidence metrics
- `scan-coverage-breakdown.*`: scan trust and mixed-repo context
- `BEFORE_AFTER.md` and `before-after-comparison.json` when `--previous-analysis` is supplied

## Product Rules

The workflow keeps the project thesis intact:

- The default lane remains capped by shared policy.
- `current_policy` remains the control arm.
- `large_file` is measured as a default-lane candidate through evidence, not promoted or demoted by intuition.
- Structural findings stay useful as backlog/watchpoint context, but engineer reports put concrete patch follow-through first.
- Generated rules are advisory until reviewed; they are not written to the target repo by default.
- LLM adjudication remains outside this workflow until static and semantic narrowing produce structured evidence bundles and treatment lift is proven.

## Completion Bar

This workflow is complete when a user can run one command against an external repo, hand the generated engineer report to that repo’s engineer, rerun after fixes, and use the before/after and evidence artifacts to decide whether the tool’s top actions were actually followed and helpful.
