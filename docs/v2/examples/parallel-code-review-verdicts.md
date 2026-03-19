# Parallel-Code Review Verdicts

Repo: `parallel-code`
Captured at: `2026-03-20T00:00:00Z`
Source report: `docs/v2/examples/parallel-code-head-engineer-report.md`

## Category Counts

- `incorrect`: 1
- `useful`: 6
- `useful_watchpoint`: 2

## Expected Trust Tiers

- `experimental`: 1
- `trusted`: 7
- `watchpoint`: 1

## Expected Presentation Classes

- `experimental`: 1
- `guarded_facade`: 1
- `hardening_note`: 1
- `structural_debt`: 4
- `tooling_debt`: 1
- `watchpoint`: 1

## Expected Leverage Classes

- `architecture_signal`: 2
- `boundary_discipline`: 1
- `experimental`: 1
- `hardening_note`: 1
- `local_refactor_target`: 1
- `regrowth_watchpoint`: 1
- `secondary_cleanup`: 1
- `tooling_debt`: 1

## Detailed Verdicts

### src/store/store.ts

- kind: `unstable_hotspot`
- category: `useful`
- report bucket: `Architecture Signals`
- expected trust tier: `trusted`
- expected presentation class: `structural_debt`
- expected leverage class: `architecture_signal`
- engineer note: The strongest architecture signal is the component-facing barrel inside the largest mixed subsystem. Fan-in, instability, and cycle position matter more than file size.
- expected v2 behavior: Keep this as a lead architecture signal and explain it as a boundary hub inside a mixed cycle, not as generic large-file debt.

### store-app mixed cycle

- kind: `cycle_cluster`
- category: `useful_watchpoint`
- report bucket: `Watchpoints`
- expected trust tier: `watchpoint`
- expected presentation class: `watchpoint`
- expected leverage class: `architecture_signal`
- engineer note: Cycle clustering is the strongest repo-wide architecture metric when it highlights mixed ownership and boundary ambiguity.
- expected v2 behavior: Keep the cycle prominent as an architecture signal, but frame it as a watchpoint with cut-candidate context rather than a mandatory refactor queue.

### src/components/TaskPanel.tsx

- kind: `dependency_sprawl`
- category: `useful`
- report bucket: `Best Local Refactor Targets`
- expected trust tier: `trusted`
- expected presentation class: `structural_debt`
- expected leverage class: `local_refactor_target`
- engineer note: This is the best contained refactor target because the repo already declares it as a guarded shell with extracted owners.
- expected v2 behavior: Surface this as a strong local refactor target when dependency breadth overlaps declared extracted-owner boundaries.

### src/lib/ipc.ts

- kind: `unstable_hotspot`
- category: `useful`
- report bucket: `Boundary Discipline`
- expected trust tier: `trusted`
- expected presentation class: `guarded_facade`
- expected leverage class: `boundary_discipline`
- engineer note: The useful question is whether domain or lifecycle policy is leaking into transport glue, not whether central fan-in alone is bad.
- expected v2 behavior: Treat guarded transport facades as boundary-discipline findings and emphasize glue-leakage risk over raw hotspot language.

### src/App.tsx

- kind: `dependency_sprawl`
- category: `useful_watchpoint`
- report bucket: `Regrowth Watchpoints`
- expected trust tier: `trusted`
- expected presentation class: `structural_debt`
- expected leverage class: `regrowth_watchpoint`
- engineer note: Composition roots naturally have broad fan-out. The value here is to keep shell ownership from regrowing, not to force major surgery.
- expected v2 behavior: Surface broad composition roots as regrowth watchpoints unless other evidence shows they are the highest-leverage architecture change.

### src/components/terminal-view/terminal-session.ts

- kind: `dependency_sprawl`
- category: `useful`
- report bucket: `Secondary Cleanup`
- expected trust tier: `trusted`
- expected presentation class: `structural_debt`
- expected leverage class: `secondary_cleanup`
- engineer note: This public lifecycle facade still has meaningful pressure, but it should read as careful secondary cleanup rather than the lead architecture problem.
- expected v2 behavior: Keep extracted-owner lifecycle facades visible as secondary cleanup when they overlap several real signals without being the highest-leverage target.

### ConnectionBannerState

- kind: `closed_domain_exhaustiveness`
- category: `useful`
- report bucket: `Targeted Hardening Notes`
- expected trust tier: `trusted`
- expected presentation class: `hardening_note`
- expected leverage class: `hardening_note`
- engineer note: The missing exhaustiveness is real, but it belongs in targeted hardening because the lifecycle model already exists elsewhere.
- expected v2 behavior: Keep narrow exhaustiveness findings visible as hardening notes and do not let them lead the architecture summary.

### scripts/session-stress.mjs

- kind: `large_file`
- category: `useful`
- report bucket: `Tooling Debt`
- expected trust tier: `trusted`
- expected presentation class: `tooling_debt`
- expected leverage class: `tooling_debt`
- engineer note: The script is a real maintenance burden, but it should not compete directly with app/runtime architecture priorities.
- expected v2 behavior: Separate script and tooling pressure into tooling debt so it stays visible without distorting the main architectural story.

### dead private code clusters

- kind: `dead_private_code_cluster`
- category: `incorrect`
- report bucket: `Experimental Side Channel`
- expected trust tier: `experimental`
- expected presentation class: `experimental`
- expected leverage class: `experimental`
- engineer note: This detector still claims live helpers are stale, so it should not influence maintainer-facing prioritization.
- expected v2 behavior: Keep this detector quarantined as experimental until same-file and TSX helper usage are reliable.

