# Sentrux Public Beta

This repository is the public beta for `yshaaban/sentrux`.

The current public beta is centered on one job:

- help a coding agent or reviewer catch fixable patch-safety and structural issues before they land

The public promise is intentionally narrower than "general code-quality review for every repo." The product is strongest when you use it to review an active patch, understand missing follow-through, and decide what to fix first.

## What To Use First

- Start with the root [README](../README.md) for install and command examples.
- Use `sentrux gate`, `sentrux brief`, or MCP `check` if you want the current v2 patch-safety loop.
- Use `sentrux check` only if you specifically need the legacy structural-rules lane.

Good current output should usually feel like this:

- a small number of high-trust issues
- clear obligations when the patch changed a shared concept or closed domain
- enough repair guidance that the next edit is obvious
- an easy rerun path to confirm the patch improved

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

What the current beta emphasizes:

- trustworthy top findings over broad detector count
- patch-level review value over repo-level score storytelling
- repair guidance over raw metric volume
- public-proof discipline before broader promotion of detectors

What is still intentionally limited:

- broad repo-to-repo consistency outside the current proof corpus
- deep semantic review across every ecosystem
- treating every structurally true issue as a top-priority action

## Known Limitations

- Public platform support is narrower than the full long-term product target.
- Some release-time assets still depend on grammar bundles that are produced during release packaging.
- Larger real-repo calibration, benchmark regression gating on dedicated quiet runners, and broader unhappy-path validation are still maintainer work in progress.
- The public beta should be treated as a fast-moving feedback surface, not as a stability promise for every detector family.

## What Feedback Is Most Useful

- incorrect or confusing `gate`, `brief`, or MCP `check` output
- top-ranked findings that were technically true but not worth fixing first
- findings that were right but did not give enough guidance to repair
- missing high-value issues that a strong reviewer would have prioritized
- false positives with a public-safe reproduction
- install or upgrade failures on the supported matrix
- GUI startup failures, especially Linux GPU fallback problems
- missing documentation or misleading product positioning

If you file feedback on findings quality, the most useful format is:

1. what the tool ranked near the top
2. what you expected to see instead
3. whether the surfaced issue was worth fixing
4. whether the fix path was clear from the output

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
