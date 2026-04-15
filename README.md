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

[Quick Start](#quick-start) · [Support Matrix](#support-matrix) · [MCP](#mcp-integration) · [Public Beta](#public-beta) · [Documentation](#documentation) · [Releases](https://github.com/yshaaban/sentrux/releases)

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

- Public beta guide: [docs/public-beta.md](docs/public-beta.md)
- Privacy and telemetry: [docs/privacy-and-telemetry.md](docs/privacy-and-telemetry.md)
- Contributing: [CONTRIBUTING.md](CONTRIBUTING.md)
- Security reporting: [SECURITY.md](SECURITY.md)
- Current v2 maintainer docs: [docs/v2/README.md](docs/v2/README.md)
- Current implementation audit: [docs/v2/implementation-status.md](docs/v2/implementation-status.md)
- Public release checklist: [docs/v2/release-checklist.md](docs/v2/release-checklist.md)
- Changelog: [CHANGELOG.md](CHANGELOG.md)
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
