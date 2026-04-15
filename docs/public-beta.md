# Sentrux Public Beta

This repository is the public beta for `yshaaban/sentrux`.

## What To Use First

- Start with the root [README](../README.md) for install and command examples.
- Use `sentrux gate`, `sentrux brief`, or MCP `check` if you want the current v2 patch-safety loop.
- Use `sentrux check` only if you specifically need the legacy structural-rules lane.

## Supported Public Beta Paths

- `macOS arm64`: official release binary and `install.sh`
- `Linux x86_64`: official release binary and `install.sh`
- `Linux aarch64`: official release binary and `install.sh`
- `macOS x86_64`: source build only
- `Windows`: not part of the public beta support matrix yet

The public release source of truth is this repository, `yshaaban/sentrux`. Official binaries, `install.sh`, and release notes should all point back to this public repo.

## Current Boundaries

- The GUI, `gate`, `brief`, and MCP patch-safety surface are the main public beta workflows.
- Some detector families remain intentionally `experimental` and stay quarantined out of default top-level decisions until the evidence base is stronger.
- Deep maintainer calibration, benchmark, and eval material lives under [docs/v2/](./v2/README.md).
- The public repo should stay free of private benchmark artifacts, private repo names, internal-only links, and workstation-specific paths.

## Known Limitations

- Public platform support is narrower than the full long-term product target.
- Some release-time assets still depend on grammar bundles that are produced during release packaging.
- Larger real-repo calibration, benchmark regression gating on dedicated quiet runners, and broader unhappy-path validation are still maintainer work in progress.
- The public beta should be treated as a fast-moving feedback surface, not as a stability promise for every detector family.

## What Feedback Is Most Useful

- incorrect or confusing `gate`, `brief`, or MCP `check` output
- false positives with a public-safe reproduction
- install or upgrade failures on the supported matrix
- GUI startup failures, especially Linux GPU fallback problems
- missing documentation or misleading product positioning

## What To Attach Safely

- sanitized logs
- a small public reproduction repo or reduced patch
- your operating system, architecture, install method, and exact command

Do not attach:

- private repository code
- secrets, tokens, or customer data
- internal links, internal hostnames, or workstation-specific absolute paths

## Where To Report Things

- bugs and features: GitHub Issues in this repo
- public test feedback: use the public-test issue template
- security concerns: follow [../SECURITY.md](../SECURITY.md)
