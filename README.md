<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="assets/logo-dark.svg">
  <source media="(prefers-color-scheme: light)" srcset="assets/logo-light.svg">
  <img alt="sentrux" src="assets/logo-dark.svg" width="200">
</picture>

<br><br>

**Your AI agent writes the code.<br>sentrux tells you what it did to your architecture.**

<br>

[![CI](https://github.com/sentrux/sentrux/actions/workflows/ci.yml/badge.svg)](https://github.com/sentrux/sentrux/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/sentrux/sentrux)](https://github.com/sentrux/sentrux/releases)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Stars](https://img.shields.io/github/stars/sentrux/sentrux?style=flat)](https://github.com/sentrux/sentrux/stargazers)

[Install](#install) · [Quick Start](#quick-start) · [MCP Integration](#mcp-server) · [Rules Engine](#rules-engine) · [Releases](https://github.com/sentrux/sentrux/releases)

</div>

<br>

<div align="center">

![sentrux demo](assets/demo.gif)

</div>

<div align="center">
<sub>One prompt. One AI agent. Five minutes. <b>Health Grade: D.</b></sub>
<br>
<sub>Watch Claude Code build a FastAPI project from scratch — while sentrux shows the architecture decaying in real-time.</sub>
</div>

<details>
<summary>See the final grade report of this demo project</summary>
<br>
<table>
<tr>
<td align="center"><img src="assets/grade-health.png" width="240" alt="Health Grade D"><br><b>Health: D</b><br><sub>cohesion F, dead code F (25%)<br>comments D (2%)</sub></td>
<td align="center"><img src="assets/grade-architecture.png" width="240" alt="Architecture Grade B"><br><b>Architecture: B</b><br><sub>levelization A, distance A<br>blast radius B (23 files)</sub></td>
<td align="center"><img src="assets/grade-test-coverage.png" width="240" alt="Test Coverage Grade D"><br><b>Test Coverage: D</b><br><sub>38% coverage<br>42 untested files</sub></td>
</tr>
</table>
</details>

<br>

## The problem nobody talks about

You start a project with Claude Code or Cursor. Day one is magic. The agent writes clean code, understands your intent, ships features fast.

Then something shifts.

The agent starts hallucinating functions that don't exist. It puts new code in the wrong place. It introduces bugs in files it touched yesterday. You ask for a simple feature and it breaks three other things. You're spending more time fixing the agent's output than writing it yourself.

Everyone assumes the AI got worse. **It didn't.** Your codebase did.

Every AI session silently degrades your architecture. Same function names, different purposes, scattered across files. Unrelated code dumped in the same folder. Dependencies tangling into spaghetti. When the agent searches your project, it finds twenty conflicting matches — and picks the wrong one. Every session makes the mess worse. Every mess makes the next session harder.

This is the dirty secret of AI-assisted development: **the better the AI generates code, the faster your codebase becomes ungovernable.**

Nobody planned for this. The traditional answer — *"design your architecture first"* — assumes you know what you're building before you build it. But that's not how anyone actually works with AI agents. You prototype. You iterate. You follow inspiration. You let the conversation drive the code.

That creative flow is exactly what makes AI agents powerful. And it's exactly what destroys codebases.

## The solution

**sentrux is the missing feedback loop.**

It watches your codebase in real-time — not the diffs, not the terminal output — the *actual architecture*. Every file. Every dependency. Every structural relationship. Visualized as a live interactive treemap that updates as the agent writes code.

14 health dimensions. Graded A through F. Computed in milliseconds.

When architecture degrades, you see it immediately — not two weeks later when everything is broken and nobody remembers which session caused it.

<br>

<div align="center">
<table>
<tr>
<td align="center" width="33%"><b>Visualize</b><br><sub>Live treemap with dependency edges,<br>files glow when the agent modifies them</sub></td>
<td align="center" width="33%"><b>Measure</b><br><sub>14 health dimensions graded A-F:<br>coupling, cycles, cohesion, dead code...</sub></td>
<td align="center" width="33%"><b>Govern</b><br><sub>Quality gate catches regression.<br>Rules engine enforces constraints.</sub></td>
</tr>
</table>
</div>

<br>

## Install

```bash
brew install sentrux/tap/sentrux
```

```bash
# or: macOS / Linux
curl -fsSL https://raw.githubusercontent.com/sentrux/sentrux/main/install.sh | sh
```

Pure Rust. Single binary. No runtime dependencies. 23 languages via tree-sitter.

<details>
<summary>From source / upgrade</summary>

```bash
# Build from source
git clone https://github.com/sentrux/sentrux.git
cd sentrux && cargo build --release

# Upgrade
brew update && brew upgrade sentrux
# or re-run the curl install — it always pulls the latest release
```

</details>

## Quick start

```bash
sentrux                    # open the GUI — live treemap of your project
sentrux check .            # check rules (CI-friendly, exits 0 or 1)
sentrux gate --save .      # save baseline before agent session
sentrux gate .             # compare after — catches degradation
```

## MCP server

sentrux runs as an [MCP](https://modelcontextprotocol.io) server — your AI agent can query structural health mid-session.

```json
{
  "sentrux": {
    "command": "sentrux",
    "args": ["--mcp"]
  }
}
```

Works with Claude Code, Cursor, Windsurf, and any MCP-compatible agent.

<details>
<summary>See the agent workflow</summary>

```
Agent: scan("/Users/me/myproject")
  → { structure_grade: "B", architecture_grade: "B", files: 139 }

Agent: session_start()
  → { status: "Baseline saved", grade: "B" }

  ... agent writes 500 lines of code ...

Agent: session_end()
  → { pass: false, grade_before: "B", grade_after: "C",
      summary: "Architecture degraded during this session" }
```

15 tools: `scan` · `health` · `architecture` · `coupling` · `cycles` · `hottest` · `evolution` · `dsm` · `test_gaps` · `check_rules` · `session_start` · `session_end` · `rescan` · `blast_radius` · `level`

</details>

## Rules engine

Define architectural constraints. Enforce them in CI. Let the agent know the boundaries.

<details>
<summary>Example .sentrux/rules.toml</summary>

```toml
[constraints]
max_cycles = 0
max_coupling = "B"
max_cc = 25
no_god_files = true

[[layers]]
name = "core"
paths = ["src/core/*"]
order = 0

[[layers]]
name = "app"
paths = ["src/app/*"]
order = 2

[[boundaries]]
from = "src/app/*"
to = "src/core/internal/*"
reason = "App must not depend on core internals"
```

```bash
sentrux check .
# ✓ All rules pass — Structure: B  Architecture: B
```

</details>

## Supported languages

Rust · Python · JavaScript · TypeScript · Go · C · C++ · Java · Ruby · C# · PHP · Bash · HTML · CSS · SCSS · Swift · Lua · Scala · Elixir · Haskell · Zig · R · OCaml

---

<div align="center">

<sub>AI agents write code at machine speed. Without structural governance, codebases decay at machine speed too.<br><b>sentrux is the governor.</b></sub>

</div>

## License

[MIT](LICENSE)
