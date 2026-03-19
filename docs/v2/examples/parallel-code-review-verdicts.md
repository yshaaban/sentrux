# Parallel-Code Review Verdicts

Repo: `parallel-code`
Captured at: `2026-03-19`
Source report: `docs/v2/examples/parallel-code-live-engineer-report.md`

## Category Counts

- `incorrect`: 1
- `low_value`: 1
- `real_but_overstated`: 2
- `useful`: 1
- `useful_watchpoint`: 3

## Expected Trust Tiers

- `experimental`: 1
- `trusted`: 3
- `watchpoint`: 4

## Detailed Verdicts

### task_presentation_status

- kind: `closed_domain_exhaustiveness`
- category: `real_but_overstated`
- report bucket: `hardening`
- expected trust tier: `trusted`
- engineer note: The file is already the canonical task presentation model with direct tests. An explicit total mapping could still help, but it is not a top architecture issue.
- expected v2 behavior: Keep as an objective hardening signal and avoid ranking it as repo-roadmap-critical.

### ConnectionBannerState

- kind: `closed_domain_exhaustiveness`
- category: `real_but_overstated`
- report bucket: `hardening`
- expected trust tier: `trusted`
- engineer note: Runtime handling is already exhaustive. The remaining gap is presentation grouping in App.tsx rather than a high-severity lifecycle bug.
- expected v2 behavior: Keep as presentation hardening evidence instead of a high-severity architecture signal.

### electron/remote/ws-server.ts :: server/browser-websocket.ts

- kind: `clone_family`
- category: `useful_watchpoint`
- report bucket: `watchpoint`
- expected trust tier: `watchpoint`
- engineer note: The pair is no longer symmetric enough to recommend helper extraction by default. Shared contract tests are a better first response.
- expected v2 behavior: Surface as clone drift evidence with a watchpoint classification, not as a top dedupe target.

### src/components/AgentGlyph.tsx :: src/remote/RemoteAgentGlyph.tsx

- kind: `clone_family`
- category: `useful`
- report bucket: `debt_signal`
- expected trust tier: `trusted`
- engineer note: This is real duplication and a strong maintainability cleanup candidate.
- expected v2 behavior: Keep as a high-confidence duplication debt signal.

### server/browser-control-plane.ts

- kind: `coordination_hotspot`
- category: `useful_watchpoint`
- report bucket: `watchpoint`
- expected trust tier: `watchpoint`
- engineer note: This seam already has architecture tests and is worth watching closely as behavior grows.
- expected v2 behavior: Keep as an evidence-backed coordination watchpoint.

### electron/ipc/hydra-adapter.ts

- kind: `coordination_hotspot`
- category: `low_value`
- report bucket: `watchpoint`
- expected trust tier: `watchpoint`
- engineer note: Less compelling than browser-control-plane as a top hotspot target in the current architecture state.
- expected v2 behavior: Downrank relative to stronger hotspot evidence without suppressing the factual hotspot metrics.

### dead_private_code_cluster

- kind: `dead_private_code_cluster`
- category: `incorrect`
- report bucket: `experimental`
- expected trust tier: `experimental`
- engineer note: The reported stale private helpers included plainly live functions in ScrollingDiffView.tsx, review.ts, and SidebarTaskRow.tsx. The detector is not accurate enough for engineer-facing guidance yet.
- expected v2 behavior: Keep the detector quarantined as experimental until the call/reference model is fixed.

### store/app cycle cluster

- kind: `cycle_cluster`
- category: `useful_watchpoint`
- report bucket: `watchpoint`
- expected trust tier: `watchpoint`
- engineer note: The large cycle is directionally plausible, but engineers still need seam-aware interpretation and a likely back-edge to cut before it becomes design guidance.
- expected v2 behavior: Keep as a watchpoint and surface cut candidates instead of implying an obvious refactor queue.

