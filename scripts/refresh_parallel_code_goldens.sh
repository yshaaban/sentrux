#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
PARALLEL_CODE_ROOT="${PARALLEL_CODE_ROOT:-<parallel-code-root>}"
RULES_SOURCE="$REPO_ROOT/docs/v2/examples/parallel-code.rules.toml"
OUTPUT_DIR="${OUTPUT_DIR:-$REPO_ROOT/docs/v2/examples/parallel-code-golden}"
SENTRUX_BIN="${SENTRUX_BIN:-$REPO_ROOT/target/debug/sentrux}"
PARALLEL_SENTRUX_DIR="$PARALLEL_CODE_ROOT/.sentrux"
PARALLEL_RULES_PATH="$PARALLEL_SENTRUX_DIR/rules.toml"
RULES_BACKUP_PATH="$PARALLEL_SENTRUX_DIR/rules.toml.bak_sentrux_tmp"

if [[ ! -x "$SENTRUX_BIN" ]]; then
  echo "Expected built sentrux binary at $SENTRUX_BIN" >&2
  exit 1
fi

if [[ ! -f "$RULES_SOURCE" ]]; then
  echo "Missing example rules file at $RULES_SOURCE" >&2
  exit 1
fi

if [[ ! -d "$PARALLEL_CODE_ROOT" ]]; then
  echo "Missing parallel-code repo at $PARALLEL_CODE_ROOT" >&2
  exit 1
fi

if ! command -v node >/dev/null 2>&1; then
  echo "This script requires node on PATH" >&2
  exit 1
fi

tmpdir="$(mktemp -d)"

cleanup() {
  rm -rf "$tmpdir"
  if [[ -f "$RULES_BACKUP_PATH" ]]; then
    mv "$RULES_BACKUP_PATH" "$PARALLEL_RULES_PATH"
  else
    rm -f "$PARALLEL_RULES_PATH"
  fi
}

trap cleanup EXIT

mkdir -p "$PARALLEL_SENTRUX_DIR"
mkdir -p "$OUTPUT_DIR"

if [[ -f "$PARALLEL_RULES_PATH" ]]; then
  cp "$PARALLEL_RULES_PATH" "$RULES_BACKUP_PATH"
fi
cp "$RULES_SOURCE" "$PARALLEL_RULES_PATH"

cat > "$tmpdir/requests.jsonl" <<EOF
{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"scan","arguments":{"path":"$PARALLEL_CODE_ROOT"}}}
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"concepts","arguments":{}}}
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"findings","arguments":{"limit":12}}}
{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"explain_concept","arguments":{"id":"task_git_status"}}}
{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"explain_concept","arguments":{"id":"task_presentation_status"}}}
{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"explain_concept","arguments":{"id":"server_state_bootstrap"}}}
{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"obligations","arguments":{"concept":"task_presentation_status"}}}
{"jsonrpc":"2.0","id":8,"method":"tools/call","params":{"name":"parity","arguments":{"contract":"server_state_bootstrap"}}}
{"jsonrpc":"2.0","id":9,"method":"tools/call","params":{"name":"state","arguments":{}}}
EOF

"$SENTRUX_BIN" --mcp < "$tmpdir/requests.jsonl" | grep '^[{]' > "$tmpdir/responses.jsonl"

node - "$tmpdir/responses.jsonl" "$OUTPUT_DIR" <<'EOF'
const fs = require('node:fs');
const path = require('node:path');

const [, , responsesPath, outputDir] = process.argv;
const responseLines = fs
  .readFileSync(responsesPath, 'utf8')
  .split('\n')
  .map((line) => line.trim())
  .filter(Boolean);

const responses = new Map();
for (const line of responseLines) {
  const payload = JSON.parse(line);
  responses.set(payload.id, payload);
}

const outputs = [
  [1, 'scan.json'],
  [2, 'concepts.json'],
  [3, 'findings-top12.json'],
  [4, 'explain-task_git_status.json'],
  [5, 'explain-task_presentation_status.json'],
  [6, 'explain-server_state_bootstrap.json'],
  [7, 'obligations-task_presentation_status.json'],
  [8, 'parity-server_state_bootstrap.json'],
  [9, 'state.json'],
];

for (const [id, filename] of outputs) {
  const response = responses.get(id);
  if (!response) {
    throw new Error(`Missing MCP response for id ${id}`);
  }

  const text = response.result?.content?.[0]?.text;
  if (typeof text !== 'string') {
    throw new Error(`Missing text payload for id ${id}`);
  }

  const parsed = JSON.parse(text);
  fs.writeFileSync(path.join(outputDir, filename), `${JSON.stringify(parsed, null, 2)}\n`);
}
EOF

cat > "$OUTPUT_DIR/metadata.json" <<EOF
{
  "generated_at": "$(date -u +"%Y-%m-%dT%H:%M:%SZ")",
  "parallel_code_root": "$PARALLEL_CODE_ROOT",
  "rules_source": "$RULES_SOURCE",
  "sentrux_binary": "$SENTRUX_BIN"
}
EOF

echo "Wrote scoped parallel-code goldens to $OUTPUT_DIR"
