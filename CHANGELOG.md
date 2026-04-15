# Changelog

All notable changes to this repository should be documented in this file.

## Unreleased

### Added

- MCP v2 patch-safety surfaces centered on `check`, `agent_brief`, `findings`, `obligations`, `gate`, and richer `session_end` output
- checked-in multi-repo validation artifacts for `parallel-code`, `private-benchmark-repo`, and `private-frontend`
- defect-injection, remediation-eval, review-packet, session-telemetry, and signal-scorecard tooling under `scripts/evals/`
- archetype-aware onboarding helpers, starter rules, and project-shape output for adoption flows

### Changed

- `sentrux gate` now carries the touched-concept v2 path when v2 rules are configured
- `sentrux brief` exposes structured v2 guidance JSON for `repo_onboarding`, `patch`, and `pre_merge`
- trusted findings, watchpoints, and experimental detectors are separated more explicitly across the v2 reporting surfaces
- public release documentation now reflects the actual support matrix and current CLI versus MCP surface split
- stale planning notes and design prototypes moved out of the public docs index into `docs/archive/`

### Fixed

- patch-safety analysis reuse across `gate` and `session_end`
- stronger confidence handling for copied, incompatible, or cross-project baselines
- cleaner production-first clone-drift and structural-debt reporting across benchmark repos
