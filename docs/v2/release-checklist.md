# Sentrux Public Release Checklist

Use this checklist before publishing a public release, not only before cutting an internal v2 proof refresh.

## 1. Documentation And Messaging

- update [`../../README.md`](../../README.md) so install instructions, CLI/MCP positioning, and examples match the shipping product
- update [`../../CHANGELOG.md`](../../CHANGELOG.md) with the release highlights
- verify [`./README.md`](./README.md), [`./implementation-status.md`](./implementation-status.md), and [`./roadmap.md`](./roadmap.md) were re-audited recently
- make sure docs do not claim unsupported binaries or unsupported Homebrew targets
- make sure docs do not present CLI `sentrux check` as the v2 fast-path patch surface
- keep only maintained docs in the public index and move stale planning or prototype material into [`../archive/`](../archive/README.md)

## 2. Artifact And Support-Matrix Truth

Current expected public matrix:

- GitHub release artifacts: `macOS arm64`, `Linux x86_64`, `Linux aarch64`, `Windows x86_64`
- Homebrew formula: `macOS arm64`, `Linux x86_64`
- `install.sh`: `macOS arm64`, `Linux x86_64`, `Linux aarch64`
- `macOS x86_64`: source build only

Before release:

- confirm [`../../install.sh`](../../install.sh) still matches the actual uploaded artifacts
- confirm [`.github/workflows/release.yml`](../../.github/workflows/release.yml) still matches the public support matrix
- if the support matrix changed, update docs before tagging

## 3. Core Validation Commands

Run:

1. `cargo fmt --all --check`
2. `cargo test -p sentrux-core -- --nocapture`
3. `cargo build -p sentrux`
4. `npm --prefix ts-bridge test`
5. `git diff --check`

## 4. Benchmark And Golden Validation

Run:

1. `node scripts/validate_parallel_code_v2.mjs`
2. `node scripts/validate_h1_sdk_v2.mjs`
3. `node scripts/validate_admin_frontend_v2.mjs`
4. `node scripts/validate_benchmark_repos_v2.mjs`

Review:

- whether the checked-in goldens changed intentionally
- whether benchmark comparisons remain comparable
- whether any fail-tier regression crosses the current policy

Benchmark policy:

- fail-tier regression: more than `250ms` and more than `20%`
- warn-tier regression: more than `150ms` and more than `10%`
- warn-tier regressions need explanation before release even if they do not block

## 5. Eval And Signal-Quality Artifacts

- refresh or verify the checked-in review packets, verdicts, scorecards, defect-injection artifacts, and remediation artifacts that are meant to ship as reference evidence
- confirm the repo-local or checked-in calibration artifacts still describe the current detector mix
- if a detector was promoted or demoted, update the documented trust-tier expectations

## 6. Baselines, Migration, And CLI/MCP Consistency

- verify incompatible or cross-project baselines are rejected clearly
- verify missing v2 baselines degrade cleanly where fallback behavior is expected
- verify `sentrux gate` and MCP `gate` still agree on the touched-concept story for representative repos
- verify MCP `check` still returns ranked actions and fast-path availability details
- verify CLI `sentrux brief` still emits valid structured JSON for `repo_onboarding`, `patch`, and `pre_merge`

## 7. Packaging And Install Smoke

- smoke-test the published binary or a locally built equivalent on each supported platform path you intend to advertise
- smoke-test Homebrew separately from raw release downloads because the supported platform set is narrower
- on Linux, confirm the GUI still starts on at least one Vulkan path and one OpenGL fallback path if relevant to the release

## 8. Release Operations

- confirm the tag will be built from the intended commit
- confirm the release workflow secrets exist and are current: `PRO_REPO_TOKEN`, `HOMEBREW_TAP_TOKEN`
- remember that the release workflow currently builds through the private `private-integration-crate` checkout; if that dependency changes, update the checklist and workflow together

## 9. Exit Bar

Do not tag the release unless:

- the public docs are truthful
- the support matrix is truthful
- the validation commands passed or have documented exceptions
- benchmark deltas are understood
- the current top findings on the benchmark repos still look trustworthy
- the release operator can explain what is new, what is still legacy, and what is still intentionally beta-quality
