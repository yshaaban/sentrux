# Changelog

All notable changes to this repository should be documented in this file.

## Unreleased

### Added

- MCP v2 patch-safety surfaces centered on `check`, `agent_brief`, `findings`, `obligations`, `gate`, and richer `session_end` output
- checked-in public validation artifacts for `parallel-code` plus dogfood calibration artifacts for `sentrux`
- defect-injection, remediation-eval, review-packet, session-telemetry, and signal-scorecard tooling under `scripts/evals/`
- archetype-aware onboarding helpers, starter rules, and project-shape output for adoption flows
- public beta governance and support docs: `CONTRIBUTING.md`, `SECURITY.md`, `CODE_OF_CONDUCT.md`, issue templates, and PR template
- public release hygiene scanning plus one-command public release preflight automation

### Changed

- `sentrux gate` now carries the touched-concept v2 path when v2 rules are configured
- `sentrux brief` exposes structured v2 guidance JSON for `repo_onboarding`, `patch`, and `pre_merge`
- trusted findings, watchpoints, and experimental detectors are separated more explicitly across the v2 reporting surfaces
- public release documentation now reflects the actual support matrix, the current CLI versus MCP surface split, and the maintained public repo at `yshaaban/sentrux`
- stale planning notes and design prototypes moved out of the public docs index into `docs/archive/`
- release packaging now builds from the public tree only and publishes only the supported public binary matrix
- checked-in benchmark and golden refresh flows now use disposable frozen clones for stable public artifacts, while fail-tier benchmark gating is documented as a dedicated quiet-runner step rather than a noisy local preflight step

### Fixed

- patch-safety analysis reuse across `gate` and `session_end`
- stronger confidence handling for copied, incompatible, or cross-project baselines
- cleaner production-first clone-drift and structural-debt reporting across benchmark repos
- benchmark and golden determinism for age-sensitive clone/evolution signals by pinning proof runs to the analyzed commit epoch
- public docs and checked-in artifacts no longer carry abandoned upstream repo links, private release dependencies, internal repo names, or maintainer workstation paths
