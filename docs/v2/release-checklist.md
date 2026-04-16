# Sentrux Public Release Checklist

Use this checklist before publishing a public release, not only before cutting an internal v2 proof refresh.

## 1. Documentation And Messaging

- update [`../../README.md`](../../README.md) so install instructions, CLI/MCP positioning, and examples match the shipping product
- update [`../../CHANGELOG.md`](../../CHANGELOG.md) with the release highlights
- verify [`./README.md`](./README.md), [`../public-beta.md`](../public-beta.md), [`../privacy-and-telemetry.md`](../privacy-and-telemetry.md), [`./implementation-status.md`](./implementation-status.md), and [`./roadmap.md`](./roadmap.md) were re-audited recently
- make sure docs do not claim unsupported binaries or unsupported install paths
- make sure docs do not present CLI `sentrux check` as the v2 fast-path patch surface
- keep only maintained docs in the public index and move stale planning or prototype material into [`../archive/`](../archive/README.md)
- confirm the canonical public repo and release links point to `yshaaban/sentrux`

## 2. Artifact And Support-Matrix Truth

Current expected public matrix:

- GitHub release artifacts: `macOS arm64`, `Linux x86_64`, `Linux aarch64`
- `install.sh`: `macOS arm64`, `Linux x86_64`, `Linux aarch64`
- `macOS x86_64`: source build only
- `Windows`: not in the public beta support matrix

Before release:

- confirm [`../../install.sh`](../../install.sh) still matches the actual uploaded artifacts
- confirm [`.github/workflows/release.yml`](../../.github/workflows/release.yml) still matches the public support matrix
- confirm [`.github/workflows/ci.yml`](../../.github/workflows/ci.yml) validates the same supported public paths you advertise
- confirm grammar bundle inputs are pinned to exact upstream commits rather than moving branch names
- if the support matrix changed, update docs before tagging

## 3. Core Validation Commands

Prerequisite:

- make sure the public `parallel-code` repo is available at `../parallel-code`, or set `PARALLEL_CODE_ROOT` before running the preflight

Run:

1. `node scripts/release_preflight_public.mjs`

Or, if you need to run the lanes manually:

1. `cargo fmt --all --check`
2. `npm ci --prefix ts-bridge`
3. `cargo test -p sentrux-core -- --nocapture`
4. `cargo build -p sentrux`
5. `cargo build --release -p sentrux`
6. `npm --prefix ts-bridge test`
7. `bash ./scripts/install_tree_sitter_cli.sh`
8. `./scripts/build_grammar_bundle.sh --platform <current-platform-bundle> --output <tmp-bundle-path>`
9. `./scripts/smoke_test_install.sh --artifact-path target/release/sentrux --artifact-name <current-platform-artifact> --grammar-bundle-path <tmp-bundle-path>`
10. `git diff --check`
11. `git diff --exit-code`

The local public preflight is intentionally read-only for tracked repo content. It does not regenerate benchmark artifacts, and it fails if tracked files change during the run.

## 4. Benchmark And Golden Validation

Run:

1. `node scripts/validate_parallel_code_v2.mjs --goldens-only`
2. `node scripts/check_public_release_hygiene.mjs`

Run the benchmark regression gate only on a quiet machine or dedicated CI runner:

1. `node scripts/validate_benchmark_repos_v2.mjs`

Review:

- whether the checked-in goldens changed intentionally
- whether benchmark comparisons remain comparable
- whether any fail-tier regression on the dedicated benchmark runner crosses the current policy
- whether any public artifact still embeds internal repo names or workstation-specific paths
- whether any informational self-benchmark docs such as [`./examples/sentrux-benchmark.json`](./examples/sentrux-benchmark.json) need a deliberate refresh outside the clean-tree preflight path

Benchmark policy:

- fail-tier regression: more than `250ms` and more than `20%`
- warn-tier regression: more than `150ms` and more than `10%`
- warn-tier regressions need explanation before release even if they do not block

## 5. Eval And Signal-Quality Artifacts

- refresh or verify the checked-in review packets, verdicts, scorecards, defect-injection artifacts, and remediation artifacts that are meant to ship as reference evidence
- confirm the repo-local or checked-in calibration artifacts still describe the current detector mix
- verify the current scorecard still meets the actionable-ranking policy for the lead surface:
  - top-1 actionable precision
  - top-3 actionable precision
  - top-10 actionable precision when enough curated samples exist
  - ranking-preference satisfaction when verdicts include `preferred_over`
- verify review packets still show adequate repair-packet completeness for the top-ranked samples; this is packet-local evidence today, so do not silently treat missing fix guidance as scorecard-grade pass
- if a detector was promoted or demoted, update the documented trust-tier expectations

## 6. Baselines, Migration, And CLI/MCP Consistency

- verify incompatible or cross-project baselines are rejected clearly
- verify missing v2 baselines degrade cleanly where fallback behavior is expected
- verify `sentrux gate` and MCP `gate` still agree on the touched-concept story for representative repos
- verify MCP `check` still returns ranked actions and fast-path availability details
- verify CLI `sentrux brief` still emits valid structured JSON for `repo_onboarding`, `patch`, and `pre_merge`

## 7. Packaging And Install Smoke

- smoke-test the published binary or a locally built equivalent on each supported platform path you intend to advertise
- remember that [`../../scripts/release_preflight_public.mjs`](../../scripts/release_preflight_public.mjs) now installs `ts-bridge` dependencies, installs the pinned `tree-sitter` CLI if needed, builds the current-platform grammar bundle, runs the bundle-aware installer smoke path, and fails if tracked files change
- run [`../../scripts/smoke_test_install.sh`](../../scripts/smoke_test_install.sh) directly when you need to debug installer or packaging changes in isolation
- confirm the release workflow now exercises both local bundle-consumption smoke before upload and exact-tag public install smoke after publication on every supported public platform
- on Linux, confirm the GUI still starts on at least one Vulkan path and one OpenGL fallback path if relevant to the release
- confirm first-run grammar installation works against the release you just published
- confirm the root README install snippet still works from a fresh clone or a clean machine
- keep the CI and release workflows running the installer smoke path for the supported current-platform artifact on every candidate build

## 8. Release Operations

- confirm the tag will be built from the intended commit
- confirm the release workflow builds from this public repo only
- confirm grammar bundles are attached in the same release path as the binaries
- if any manual release steps remain, document them in the release notes or maintainer docs before tagging

## 9. Governance, Privacy, And Public Support

- verify [`../../CONTRIBUTING.md`](../../CONTRIBUTING.md), [`../../SECURITY.md`](../../SECURITY.md), and [`../../CODE_OF_CONDUCT.md`](../../CODE_OF_CONDUCT.md) still match the current public process
- verify the issue templates and PR template still route public users correctly
- make sure the privacy/telemetry description in [`../privacy-and-telemetry.md`](../privacy-and-telemetry.md) is still truthful
- confirm the release operator can explain what logs and artifacts are safe for public issue reports

## 10. Exit Bar

Do not tag the release unless:

- the public docs are truthful
- the support matrix is truthful
- the validation commands passed or have documented exceptions
- the governance and privacy docs are truthful
- benchmark deltas are understood
- the current top findings on the benchmark repos still look trustworthy
- the release operator can explain what is new, what is still legacy, and what is still intentionally beta-quality
