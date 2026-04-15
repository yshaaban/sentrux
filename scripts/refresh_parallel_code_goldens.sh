#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
source "$REPO_ROOT/scripts/lib/benchmark-plugin-home.sh"
DEFAULT_PARALLEL_CODE_ROOT="$(cd "$REPO_ROOT/.." && pwd)/parallel-code"
PARALLEL_CODE_ROOT="${PARALLEL_CODE_ROOT:-$DEFAULT_PARALLEL_CODE_ROOT}"
RULES_SOURCE="$REPO_ROOT/docs/v2/examples/parallel-code.rules.toml"
OUTPUT_DIR="${OUTPUT_DIR:-$REPO_ROOT/docs/v2/examples/parallel-code-golden}"
SENTRUX_BIN="${SENTRUX_BIN:-$REPO_ROOT/target/debug/sentrux}"
ANALYSIS_MODE="${ANALYSIS_MODE:-working_tree}"
SENTRUX_SKIP_GRAMMAR_DOWNLOAD="${SENTRUX_SKIP_GRAMMAR_DOWNLOAD:-1}"

export SENTRUX_SKIP_GRAMMAR_DOWNLOAD

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

if ! command -v git >/dev/null 2>&1; then
  echo "This script requires git on PATH" >&2
  exit 1
fi

tmpdir="$(mktemp -d)"
WORK_ROOT="$tmpdir/parallel-code"
WORK_SENTRUX_DIR="$WORK_ROOT/.sentrux"
WORK_RULES_PATH="$WORK_SENTRUX_DIR/rules.toml"
PLUGIN_HOME="$(prepare_typescript_benchmark_home "$tmpdir")"

cleanup() {
  rm -rf "$tmpdir"
}

trap cleanup EXIT

mkdir -p "$OUTPUT_DIR"
git clone --quiet --local --no-hardlinks "$PARALLEL_CODE_ROOT" "$WORK_ROOT"
if [[ "$ANALYSIS_MODE" == "working_tree" ]]; then
  node - "$PARALLEL_CODE_ROOT" "$WORK_ROOT" "$REPO_ROOT" <<'EOF'
const path = require('node:path');

async function main() {
  const [, , sourceRoot, targetRoot, repoRoot] = process.argv;
  const { overlayWorkingTreeChanges } = await import(
    path.resolve(repoRoot, 'scripts/lib/repo-identity.mjs')
  );
  await overlayWorkingTreeChanges({ sourceRoot, targetRoot });
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
});
EOF
elif [[ "$ANALYSIS_MODE" != "head_clone" ]]; then
  echo "Unsupported ANALYSIS_MODE: $ANALYSIS_MODE (expected working_tree or head_clone)" >&2
  exit 1
fi

mkdir -p "$WORK_SENTRUX_DIR"
cp "$RULES_SOURCE" "$WORK_RULES_PATH"

cat > "$tmpdir/requests.jsonl" <<EOF
{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"scan","arguments":{"path":"$WORK_ROOT"}}}
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"concepts","arguments":{}}}
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"findings","arguments":{"limit":12}}}
{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"explain_concept","arguments":{"id":"task_git_status"}}}
{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"explain_concept","arguments":{"id":"task_presentation_status"}}}
{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"explain_concept","arguments":{"id":"server_state_bootstrap"}}}
{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"obligations","arguments":{"concept":"task_presentation_status"}}}
{"jsonrpc":"2.0","id":8,"method":"tools/call","params":{"name":"parity","arguments":{"contract":"server_state_bootstrap"}}}
{"jsonrpc":"2.0","id":9,"method":"tools/call","params":{"name":"state","arguments":{}}}
{"jsonrpc":"2.0","id":10,"method":"tools/call","params":{"name":"agent_brief","arguments":{"mode":"repo_onboarding","limit":3}}}
{"jsonrpc":"2.0","id":11,"method":"tools/call","params":{"name":"session_start","arguments":{}}}
{"jsonrpc":"2.0","id":12,"method":"tools/call","params":{"name":"agent_brief","arguments":{"mode":"patch","limit":3}}}
{"jsonrpc":"2.0","id":13,"method":"tools/call","params":{"name":"gate","arguments":{}}}
{"jsonrpc":"2.0","id":14,"method":"tools/call","params":{"name":"agent_brief","arguments":{"mode":"pre_merge","limit":3}}}
{"jsonrpc":"2.0","id":15,"method":"tools/call","params":{"name":"session_end","arguments":{}}}
EOF

HOME="$PLUGIN_HOME" "$SENTRUX_BIN" --mcp < "$tmpdir/requests.jsonl" | grep '^[{]' > "$tmpdir/responses.jsonl"

node - "$tmpdir/responses.jsonl" "$OUTPUT_DIR" "$WORK_ROOT" "$PARALLEL_CODE_ROOT" <<'EOF'
const fs = require('node:fs');
const path = require('node:path');

const [, , responsesPath, outputDir, workRoot, sourceRoot] = process.argv;
const publicRepoRoot = '<parallel-code-root>';
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
  [10, 'agent-brief-onboarding.json'],
  [11, 'session-start.json'],
  [12, 'agent-brief-patch.json'],
  [13, 'gate-pass.json'],
  [14, 'agent-brief-pre-merge.json'],
  [15, 'session-end-pass.json'],
];

function sanitizeValue(value) {
  if (typeof value === 'string') {
    return value.split(workRoot).join(publicRepoRoot).split(sourceRoot).join(publicRepoRoot);
  }
  if (Array.isArray(value)) {
    return value.map(sanitizeValue);
  }
  if (value && typeof value === 'object') {
    return Object.fromEntries(
      Object.entries(value).map(([key, entry]) => [key, sanitizeValue(entry)]),
    );
  }
  return value;
}

function stabilizeValue(value) {
  if (Array.isArray(value)) {
    return value
      .map(stabilizeValue)
      .map((entry) => {
        if (
          typeof entry === 'string' &&
          /^youngest clone file was touched \d+ day\(s\) ago$/.test(entry)
        ) {
          return 'youngest clone file was touched recently';
        }
        return entry;
      });
  }
  if (value && typeof value === 'object') {
    return Object.fromEntries(
      Object.entries(value)
        .filter(
          ([key]) =>
            key !== 'age_days' &&
            key !== 'youngest_age_days' &&
            key !== 'quality_signal',
        )
        .map(([key, entry]) => [key, stabilizeValue(entry)]),
    );
  }
  return value;
}

for (const [id, filename] of outputs) {
  const response = responses.get(id);
  if (!response) {
    throw new Error(`Missing MCP response for id ${id}`);
  }

  const text = response.result?.content?.[0]?.text;
  if (typeof text !== 'string') {
    throw new Error(`Missing text payload for id ${id}`);
  }

  const parsed = stabilizeValue(sanitizeValue(JSON.parse(text)));
  fs.writeFileSync(path.join(outputDir, filename), `${JSON.stringify(parsed, null, 2)}\n`);
}
EOF

node - "$OUTPUT_DIR/metadata.json" "$PARALLEL_CODE_ROOT" "$WORK_ROOT" "$REPO_ROOT" "$RULES_SOURCE" "$SENTRUX_BIN" "$ANALYSIS_MODE" <<'EOF'
const fs = require('node:fs');
const path = require('node:path');

async function main() {
  const [
    ,
    ,
    metadataPath,
    parallelCodeRoot,
    analyzedRoot,
    repoRoot,
    rulesSource,
    sentruxBinary,
    analysisMode,
  ] = process.argv;
  const {
    assertRepoIdentityFresh,
    buildRepoFreshnessMetadata,
  } = await import(
    path.resolve(repoRoot, 'scripts/lib/repo-identity.mjs')
  );
  const freshness = buildRepoFreshnessMetadata({
    repoRoot: parallelCodeRoot,
    analyzedRoot,
    analysisMode,
    rulesSource,
    binaryPath: sentruxBinary,
  });
  const publicPathReplacements = [
    [sentruxBinary, '<sentrux-root>/target/debug/sentrux'],
    [rulesSource, '<sentrux-root>/docs/v2/examples/parallel-code.rules.toml'],
    [analyzedRoot, '<parallel-code-root>'],
    [parallelCodeRoot, '<parallel-code-root>'],
    [repoRoot, '<sentrux-root>'],
  ];
  function sanitizeValue(value) {
    if (typeof value === 'string') {
      return publicPathReplacements.reduce((current, [target, replacement]) => {
        return current.split(target).join(replacement);
      }, value);
    }
    if (Array.isArray(value)) {
      return value.map(sanitizeValue);
    }
    if (value && typeof value === 'object') {
      return Object.fromEntries(
        Object.entries(value).map(([key, entry]) => [key, sanitizeValue(entry)]),
      );
    }
    return value;
  }
  if (analysisMode === 'working_tree') {
    assertRepoIdentityFresh({
      expected: freshness.source_tree_identity,
      actual: freshness.analyzed_tree_identity,
      label: 'parallel-code working-tree mirror',
    });
  }
  const payload = sanitizeValue({
    generated_at: new Date().toISOString().replace(/\.\d{3}Z$/, 'Z'),
    parallel_code_root: parallelCodeRoot,
    analysis_mode: analysisMode,
    source_tree_identity: freshness.source_tree_identity,
    analyzed_tree_identity: freshness.analyzed_tree_identity,
    regression_mutation: {
      path: 'src/components/SidebarTaskRow.tsx',
      change: 'store.taskGitStatus = store.taskGitStatus',
    },
    rules_source: rulesSource,
    sentrux_binary: sentruxBinary,
    rules_identity: freshness.rules_identity,
    binary_identity: freshness.binary_identity,
  });

  fs.writeFileSync(metadataPath, `${JSON.stringify(payload, null, 2)}\n`);
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
});
EOF

node - "$WORK_ROOT/src/components/SidebarTaskRow.tsx" <<'EOF'
const fs = require('node:fs');

const [, , targetPath] = process.argv;
const source = fs.readFileSync(targetPath, 'utf8');
const needle = "function getPrimaryTaskAgentDef(taskId: string): AgentDef | null {\n";
const injection = `${needle}  store.taskGitStatus = store.taskGitStatus;\n`;

if (source.includes("store.taskGitStatus = store.taskGitStatus;")) {
  throw new Error(`Regression mutation already present in ${targetPath}`);
}
if (!source.includes(needle)) {
  throw new Error(`Could not find injection point in ${targetPath}`);
}

fs.writeFileSync(targetPath, source.replace(needle, injection));
EOF

cat > "$tmpdir/regression-requests.jsonl" <<EOF
{"jsonrpc":"2.0","id":20,"method":"tools/call","params":{"name":"scan","arguments":{"path":"$WORK_ROOT"}}}
{"jsonrpc":"2.0","id":21,"method":"tools/call","params":{"name":"gate","arguments":{}}}
{"jsonrpc":"2.0","id":22,"method":"tools/call","params":{"name":"session_end","arguments":{}}}
EOF

HOME="$PLUGIN_HOME" "$SENTRUX_BIN" --mcp < "$tmpdir/regression-requests.jsonl" | grep '^[{]' > "$tmpdir/regression-responses.jsonl"

node - "$tmpdir/regression-responses.jsonl" "$OUTPUT_DIR" "$WORK_ROOT" "$PARALLEL_CODE_ROOT" <<'EOF'
const fs = require('node:fs');
const path = require('node:path');

const [, , responsesPath, outputDir, workRoot, sourceRoot] = process.argv;
const publicRepoRoot = '<parallel-code-root>';
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
  [21, 'gate-fail.json'],
  [22, 'session-end-fail.json'],
];

function sanitizeValue(value) {
  if (typeof value === 'string') {
    return value.split(workRoot).join(publicRepoRoot).split(sourceRoot).join(publicRepoRoot);
  }
  if (Array.isArray(value)) {
    return value.map(sanitizeValue);
  }
  if (value && typeof value === 'object') {
    return Object.fromEntries(
      Object.entries(value).map(([key, entry]) => [key, sanitizeValue(entry)]),
    );
  }
  return value;
}

function stabilizeValue(value) {
  if (Array.isArray(value)) {
    return value
      .map(stabilizeValue)
      .map((entry) => {
        if (
          typeof entry === 'string' &&
          /^youngest clone file was touched \d+ day\(s\) ago$/.test(entry)
        ) {
          return 'youngest clone file was touched recently';
        }
        return entry;
      });
  }
  if (value && typeof value === 'object') {
    return Object.fromEntries(
      Object.entries(value)
        .filter(([key]) => key !== 'age_days' && key !== 'youngest_age_days')
        .map(([key, entry]) => [key, stabilizeValue(entry)]),
    );
  }
  return value;
}

for (const [id, filename] of outputs) {
  const response = responses.get(id);
  if (!response) {
    throw new Error(`Missing MCP response for id ${id}`);
  }

  const text = response.result?.content?.[0]?.text;
  if (typeof text !== 'string') {
    throw new Error(`Missing text payload for id ${id}`);
  }

  const parsed = stabilizeValue(sanitizeValue(JSON.parse(text)));
  fs.writeFileSync(path.join(outputDir, filename), `${JSON.stringify(parsed, null, 2)}\n`);
}
EOF

echo "Wrote scoped parallel-code goldens to $OUTPUT_DIR"
