# Contributing to Sentrux

This repository is prepared for broad public testing at `yshaaban/sentrux`.

## Before You Open An Issue

- Check whether the problem is already reported.
- Reproduce it on the latest `main` or the latest public release when possible.
- Sanitize anything you attach. Do not post:
  - private repository contents
  - credentials, tokens, or API keys
  - customer data
  - internal hostnames or internal ticket links
  - unnecessary absolute paths from your workstation

If the bug only appears against a private codebase, reduce it to a public-safe reproduction or describe the structural pattern without pasting private code.

## What To Include In A Good Report

- operating system and architecture
- install method used: release binary, `install.sh`, or source build
- exact command you ran
- expected behavior
- actual behavior
- sanitized logs or error output
- a small reproduction repo or patch, if you can share one safely

## Before You Open A Pull Request

- Keep the scope focused. Large mixed PRs are slow to review.
- Open an issue first for behavior changes, UX changes, or larger architectural work.
- Do not bundle unrelated cleanup with functional changes.
- Update docs when user-visible behavior changes.

## Local Checks

Run the checks that match your change:

```bash
cargo fmt --all --check
cargo test
git diff --check
```

If you change the Node-based eval or validation scripts, also run the relevant targeted tests under `scripts/tests/`.

If you change public docs, install flow, release workflows, or checked-in examples, also run:

```bash
node scripts/check_public_release_hygiene.mjs
```

If you change release-facing behavior, public artifacts, or benchmark/golden flows, run:

```bash
node scripts/release_preflight_public.mjs
```

On a supported current platform, that preflight now includes the local `install.sh` smoke path against `target/release/sentrux`.

If you are iterating specifically on `install.sh` or release packaging and want the packaging lane only, run:

```bash
./scripts/smoke_test_install.sh --artifact-path target/release/sentrux --artifact-name sentrux-linux-x86_64
```

If you change `ts-bridge/`, also run:

```bash
cd ts-bridge
npm test
```

## Review Expectations

- Public-testing readiness and safety take priority over broad refactors.
- Review may ask for narrower scope, clearer documentation, or stronger sanitization before merge.
- Maintainers may close issues or PRs that cannot be reviewed safely in public.

## Release And Support Boundaries

- This repo is for public-source development and public testing.
- Do not assume maintainers can debug private infrastructure, private repositories, or unpublished product surfaces from an issue report.
- If you need help, provide a public-safe reproduction first.
- Checked-in examples, benchmark artifacts, and docs must stay public-safe. Do not add private repo names, internal links, workstation-specific absolute paths, or artifacts generated from repos that cannot be published safely.
