# Privacy And Telemetry

This page describes the public behavior of the `yshaaban/sentrux` beta.

## Local By Default

- code analysis runs locally against the checkout you open
- repo-local artifacts such as baselines, MCP session logs, and eval outputs are written under `.sentrux/` only when you invoke the corresponding commands
- the public repo does not ship automatic repo-content upload as part of normal analysis

## Network Activity

Current public builds may perform:

- update checks
- anonymous aggregate usage telemetry
- downloads of release-matched grammar bundles from public GitHub release assets when required by the shipped binary

Model or provider traffic only happens through tools or providers you configure yourself.

## Opting Out

Disable anonymous aggregate usage telemetry with:

```bash
sentrux analytics off
```

Re-enable it with:

```bash
sentrux analytics on
```

## What The Maintainer Docs Mean By Telemetry

Some maintainer-facing docs under [docs/v2/](./v2/README.md) discuss session telemetry and calibration artifacts. Those documents describe maintainer evaluation and calibration loops, not hidden background upload of repo contents during ordinary use.

Those maintainer calibration artifacts are intended to stay repo-local or be checked in only after they are sanitized and confirmed public-safe.

## Safe Sharing Guidance

When filing public issues, share only sanitized material:

- remove secrets and tokens
- remove private code that you cannot publish safely
- remove workstation-specific absolute paths when they are not necessary
- reduce private-repo failures to a public-safe reproduction whenever possible
