<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="assets/logo-dark.svg?v=2">
  <source media="(prefers-color-scheme: light)" srcset="assets/logo-light.svg?v=2">
  <img alt="sentrux" src="assets/logo-dark.svg?v=2" width="500">
</picture>

<br>

**Structural feedback for AI-assisted code changes.**

[![CI](https://github.com/yshaaban/sentrux/actions/workflows/ci.yml/badge.svg)](https://github.com/yshaaban/sentrux/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/yshaaban/sentrux)](https://github.com/yshaaban/sentrux/releases)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

**English** | [中文](README.zh-CN.md) | [Deutsch](README.de.md) | [日本語](README.ja.md)

[Quick Start](#quick-start) · [Support Matrix](#support-matrix) · [MCP](#mcp-integration) · [Signals](#metrics-and-signals) · [Public Beta](#public-beta) · [Documentation](#documentation) · [Releases](https://github.com/yshaaban/sentrux/releases)

</div>

<br>

<div align="center">

![sentrux demo](assets/demo.gif)

</div>

Sentrux gives coding agents a structural feedback loop. The current public beta has three practical surfaces:

- the desktop GUI for live structural visualization
- MCP tools for patch-safety and reviewer-facing evidence
- CLI entry points for baselines, pre-merge checks, and legacy structural rules

The active public v2 surfaces are:

- MCP `check` for fast patch-safety guidance
- CLI `sentrux brief` and `sentrux gate` for structured v2 CLI workflows
- CLI `sentrux check` as the older structural-rules lane

The maintained public repository is `yshaaban/sentrux`.

## Quick Start

Preferred installs for the current public beta:

- `install.sh` on `macOS arm64`, `Linux x86_64`, and `Linux aarch64`
- direct GitHub release downloads for those same supported targets
- source build everywhere else

```bash
curl -fsSL https://raw.githubusercontent.com/yshaaban/sentrux/main/install.sh | sh
```

```bash
git clone https://github.com/yshaaban/sentrux.git
cd sentrux
cargo build --release -p sentrux
```

Common commands:

```bash
sentrux                       # GUI
sentrux gate --save .         # save touched-concept baseline
sentrux gate .                # compare current patch against the baseline
sentrux brief --mode patch .  # structured v2 patch guidance JSON
sentrux mcp                   # MCP server for AI agents
sentrux check .               # legacy structural rules check
```

If the GUI has trouble with Linux GPU drivers, try:

```bash
WGPU_BACKEND=vulkan sentrux
WGPU_BACKEND=gl sentrux
```

## Support Matrix

- `macOS arm64`: official release binary and `install.sh` are supported
- `Linux x86_64`: official release binary and `install.sh` are supported
- `Linux aarch64`: official release binary and `install.sh` are supported
- `macOS x86_64`: source build only; not part of the public beta support matrix
- `Windows`: not part of the public beta support matrix yet

## MCP Integration

Use MCP when you want the fast patch-safety loop inside an agent:

- `check`
- `agent_brief`
- `findings`
- `obligations`
- `gate`
- `session_end`

Preferred MCP server config:

```json
{
  "mcpServers": {
    "sentrux": {
      "command": "sentrux",
      "args": ["mcp"]
    }
  }
}
```

`sentrux --mcp` still works for older configs, but `sentrux mcp` is the explicit public command.

## Public Beta

What ships today:

- GUI: live treemap, dependency edges, structural panels, and export flow
- MCP v2 wedge: touched-concept patch safety, trusted findings, obligations, confidence, debt signals, and watchpoints
- CLI v2: `gate` and `brief`
- CLI legacy lane: `check`
- TypeScript-first semantic analysis through the Node bridge in [`ts-bridge/`](ts-bridge/README.md)

What is still intentionally beta-quality:

- some detector families stay quarantined as `experimental` until validation evidence is stronger
- platform support is limited to the matrix above
- maintainer calibration and eval tooling lives under [`docs/v2/`](docs/v2/README.md) and is not the first-stop guide for new users

Known limitations, feedback expectations, and public-test guidance live in [docs/public-beta.md](docs/public-beta.md).

## Metrics And Signals

Most users do not need the full internal metric catalog. They need to know what the tool is telling them when it blocks a patch or highlights follow-through risk.

If you only remember one rule, use the signals in this order:

1. Can I trust this run?
2. What is risky or incomplete?
3. Did my patch make the repo structurally worse?

Fields ending in `_0_10000` use a `0-10000` scale where `10000` is best, most complete, or most trustworthy.

The main user-facing signals are:

| Signal | What it tells you | Why it matters |
|---|---|---|
| `touched_concept_gate.decision` | Whether the changed scope is `pass`, `warn`, or `fail` under the current patch gate. | This is the top-line answer to "is this patch safe enough to move forward?" |
| `scan_trust.overall_confidence_0_10000` | How complete and trustworthy the current scan is. Higher is better. | A low-confidence run means you should treat the rest of the output as partial evidence, not a hard decision. |
| `findings` | Concrete risky, inconsistent, or incomplete changes in the patch. | This is the main review surface. It answers "what looks wrong?" |
| `obligations` and `obligation_completeness_0_10000` | Required follow-through sites implied by the changed concept or domain. | These catch partial edits, missing branches, and forgotten update sites. |
| `clone_families` and `clone_remediations` | Duplicate logic that now needs synchronized edits or extraction. | These are useful when a patch changed one copy of logic but likely missed others. |
| `debt_signals` | Trusted structural debt worth scheduling or fixing. | These help separate real cleanup work from noise. |
| `watchpoints` | Lower-confidence issues worth inspecting next. | These are review hints, not hard failures. |
| `introduced_findings` and `resolved_findings` | What your patch made worse or better relative to the baseline. | Useful for code review, PR summaries, and end-of-session handoff. |
| `signal_delta`, `coupling_change`, and `cycles_change` | Whether the patch made the overall structure worse or better relative to the saved baseline. | This gives whole-repo context even when the changed-scope check is the primary decision. |

If you want repo-level context beyond the patch, the main legacy structural metrics are:

| Metric | What it means | Why it is useful |
|---|---|---|
| `quality_signal` | Overall structural health score for the snapshot. Higher is better. | Good quick answer to "is this codebase generally getting healthier or noisier?" |
| `modularity`, `acyclicity`, `depth`, `equality`, `redundancy` | The five root-cause dimensions behind `quality_signal`. | Useful for understanding what kind of structural problem dominates the repo. |
| `coupling_score` | Harmful cross-module coupling. Lower is better. | Useful for spotting boundary erosion. |
| `circular_dep_count` | Number of dependency cycle clusters. | Useful for identifying tangles that make changes harder to reason about. |
| `coverage_ratio` and `gaps[].risk_score` | Structural test reach and risky untested areas. | Useful for deciding where missing tests matter most. |
| `hotspots` and `churn` | Frequently changing, complex parts of the repo. | Useful for prioritizing hardening and refactoring. |

The full reference, including lower-level structural fields and maintainer-only benchmark metrics, lives in [docs/metrics-and-signals.md](docs/metrics-and-signals.md).

## Privacy And Telemetry

- code analysis runs locally against your checkout
- repo-local calibration or eval artifacts are only written when you run the eval, benchmark, or calibration tooling explicitly
- the desktop app performs update checks and anonymous aggregate usage telemetry unless you opt out with `sentrux analytics off`
- repo contents are not uploaded by default
- any model or provider traffic only happens through tools or providers you configure yourself

More detail is in [docs/privacy-and-telemetry.md](docs/privacy-and-telemetry.md).

## Languages And Plugins

Sentrux ships with tree-sitter-based language plugins and a plugin workflow for adding or extending language support.

```bash
sentrux plugin list
sentrux plugin add <name>
sentrux plugin add-standard
sentrux plugin init my-lang
```

Built-in registry coverage currently spans:

| | | | | | |
|---|---|---|---|---|---|
| Bash | C | C++ | C# | Clojure | COBOL |
| Crystal | CSS | Dart | Dockerfile | Elixir | Erlang |
| F# | GDScript | GLSL | Go | Groovy | Haskell |
| HCL | HTML | Java | JavaScript | JSON | Julia |
| Kotlin | Lua | Markdown | Nim | Nix | Objective-C |
| Object Pascal | OCaml | Perl | PHP | PowerShell | Protobuf |
| Python | R | Ruby | Rust | Scala | SCSS |
| Solidity | SQL | Svelte | Swift | TOML | TypeScript |
| V | Vue | YAML | Zig | | |

## Documentation

### User Docs

- Public beta guide: [docs/public-beta.md](docs/public-beta.md)
- Metrics and signals reference: [docs/metrics-and-signals.md](docs/metrics-and-signals.md)
- Privacy and telemetry: [docs/privacy-and-telemetry.md](docs/privacy-and-telemetry.md)
- Changelog: [CHANGELOG.md](CHANGELOG.md)

### Contributor And Maintainer Docs

- Contributing: [CONTRIBUTING.md](CONTRIBUTING.md)
- Security reporting: [SECURITY.md](SECURITY.md)
- Current v2 maintainer docs: [docs/v2/README.md](docs/v2/README.md)
- Current implementation audit: [docs/v2/implementation-status.md](docs/v2/implementation-status.md)
- Public release checklist: [docs/v2/release-checklist.md](docs/v2/release-checklist.md)
- Historical planning and design material: [docs/archive/README.md](docs/archive/README.md)

## Feedback And Security

- bug reports and feature requests: [GitHub Issues](https://github.com/yshaaban/sentrux/issues)
- public test feedback: use the dedicated issue template in this repo
- security reporting guidance: [SECURITY.md](SECURITY.md)

## Philosophy

Sentrux is built around a simple idea: agent output improves faster when the feedback loop is specific, structural, and cheap to run. Tests verify behavior. Sentrux helps verify whether the patch still fits the system you are trying to keep coherent.

<div align="center">

[MIT License](LICENSE)

</div>
