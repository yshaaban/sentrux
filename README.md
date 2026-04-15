<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="assets/logo-dark.svg?v=2">
  <source media="(prefers-color-scheme: light)" srcset="assets/logo-light.svg?v=2">
  <img alt="sentrux" src="assets/logo-dark.svg?v=2" width="500">
</picture>

<br>

**Structural feedback for AI-assisted code changes.**

[![CI](https://github.com/sentrux/sentrux/actions/workflows/ci.yml/badge.svg)](https://github.com/sentrux/sentrux/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/sentrux/sentrux)](https://github.com/sentrux/sentrux/releases)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

**English** | [中文](README.zh-CN.md) | [Deutsch](README.de.md) | [日本語](README.ja.md)

[Quick Start](#quick-start) · [Support Matrix](#support-matrix) · [MCP](#mcp-integration) · [Languages](#languages-and-plugins) · [Docs](#documentation) · [Releases](https://github.com/sentrux/sentrux/releases)

</div>

<br>

<div align="center">

![sentrux demo](assets/demo.gif)

</div>

Sentrux gives coding agents a structural feedback loop. The current product has three practical surfaces:

- the desktop GUI for live structural visualization
- MCP patch-safety tools for agent loops
- CLI entry points for baselines, pre-merge checks, and legacy structural rules

The v2 patch-safety wedge is real, but the public docs now distinguish the shipping surfaces honestly:

- MCP `check` is the fast-path v2 patch surface
- CLI `sentrux brief` and `sentrux gate` are the main v2 CLI entry points
- CLI `sentrux check` is still the legacy structural rules check

## Quick Start

Preferred installs:

- Homebrew on `macOS arm64` and `Linux x86_64`
- `install.sh` on `macOS arm64`, `Linux x86_64`, and `Linux aarch64`
- GitHub release download on `Windows x86_64`
- source build on `macOS x86_64` and any unsupported target

```bash
brew install sentrux/tap/sentrux
```

```bash
curl -fsSL https://raw.githubusercontent.com/sentrux/sentrux/main/install.sh | sh
```

```bash
curl -L -o sentrux.exe https://github.com/sentrux/sentrux/releases/latest/download/sentrux-windows-x86_64.exe
```

```bash
git clone https://github.com/sentrux/sentrux.git
cd sentrux
cargo build --release -p sentrux
```

Common commands:

```bash
sentrux                       # GUI
sentrux gate --save .         # save touched-concept baseline
sentrux gate .                # compare current patch against the baseline
sentrux brief --mode patch .  # structured v2 patch guidance JSON
sentrux check .               # legacy structural rules check
```

If the GUI has trouble with Linux GPU drivers, try:

```bash
WGPU_BACKEND=vulkan sentrux
WGPU_BACKEND=gl sentrux
```

## Support Matrix

- `macOS arm64`: official release binary, `install.sh`, and Homebrew are supported
- `macOS x86_64`: no official binary yet; build from source
- `Linux x86_64`: official release binary, `install.sh`, and Homebrew are supported
- `Linux aarch64`: official release binary and `install.sh` are supported
- `Windows x86_64`: official release binary is supported
- Homebrew does not currently ship `macOS x86_64`, `Linux aarch64`, or Windows artifacts

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

## What Ships Today

- GUI: live treemap, dependency edges, structural panels, and export flow
- MCP v2 wedge: touched-concept patch safety, trusted findings, obligations, confidence, debt signals, and watchpoints
- CLI v2: `gate` and `brief`
- CLI legacy structural lane: `check`
- TypeScript-first semantic analysis through the Node bridge in [`ts-bridge/`](ts-bridge/README.md)
- calibration tooling for goldens, benchmarks, defect injection, remediation runs, and review packets under [`docs/v2/`](docs/v2/README.md)

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

- Release overview: [README.md](README.md)
- Current v2 source of truth: [docs/v2/README.md](docs/v2/README.md)
- Current implementation audit: [docs/v2/implementation-status.md](docs/v2/implementation-status.md)
- Public release checklist: [docs/v2/release-checklist.md](docs/v2/release-checklist.md)
- Changelog: [CHANGELOG.md](CHANGELOG.md)
- Historical planning and design material: [docs/archive/README.md](docs/archive/README.md)

## Philosophy

Sentrux is built around a simple idea: agent output improves faster when the feedback loop is specific, structural, and cheap to run. Tests verify behavior. Sentrux is meant to help verify whether the patch still fits the system you are trying to keep coherent.

<div align="center">

[MIT License](LICENSE)

</div>
