#!/usr/bin/env node

import { spawn } from 'node:child_process';
import { existsSync } from 'node:fs';
import { cp, mkdir, readFile, rename, rm, unlink, writeFile } from 'node:fs/promises';
import path from 'node:path';
import readline from 'node:readline';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, '..');
const parallelCodeRoot = process.env.PARALLEL_CODE_ROOT ?? '<parallel-code-root>';
const sentruxBin = process.env.SENTRUX_BIN ?? path.join(repoRoot, 'target/debug/sentrux');
const rulesSource = path.join(repoRoot, 'docs/v2/examples/parallel-code.rules.toml');
const outputPath =
  process.env.OUTPUT_PATH ?? path.join(repoRoot, 'docs/v2/examples/parallel-code-benchmark.json');
const parallelSentruxDir = path.join(parallelCodeRoot, '.sentrux');
const parallelRulesPath = path.join(parallelSentruxDir, 'rules.toml');
const parallelRulesBackupPath = path.join(parallelSentruxDir, 'rules.toml.bak_sentrux_benchmark');
const requestTimeoutMs = Number(process.env.REQUEST_TIMEOUT_MS ?? '120000');

function roundMs(value) {
  return Number(value.toFixed(1));
}

function nowMs() {
  return Number(process.hrtime.bigint()) / 1_000_000;
}

function assertPathExists(targetPath, label) {
  if (!existsSync(targetPath)) {
    throw new Error(`Missing ${label}: ${targetPath}`);
  }
}

function summarizeScan(payload) {
  return {
    files: payload.files,
    import_edges: payload.import_edges,
    quality_signal: payload.quality_signal,
    overall_confidence_0_10000: payload.scan_trust?.overall_confidence_0_10000 ?? null,
    resolved: payload.scan_trust?.resolution?.resolved ?? null,
    unresolved_internal: payload.scan_trust?.resolution?.unresolved_internal ?? null,
  };
}

function summarizeConcepts(payload) {
  return {
    configured_concept_count: payload.summary?.configured_concept_count ?? null,
    matched_guardrail_test_count: payload.summary?.matched_guardrail_test_count ?? null,
    inferred_concept_count: payload.summary?.inferred_concept_count ?? null,
  };
}

function summarizeFindings(payload) {
  return {
    clone_group_count: payload.clone_group_count ?? null,
    finding_count: Array.isArray(payload.findings) ? payload.findings.length : null,
    semantic_finding_count: payload.semantic_finding_count ?? null,
  };
}

function summarizeExplainConcept(payload) {
  return {
    finding_count: Array.isArray(payload.findings) ? payload.findings.length : null,
    obligation_count: Array.isArray(payload.obligations) ? payload.obligations.length : null,
    read_count: Array.isArray(payload.semantic?.reads) ? payload.semantic.reads.length : null,
    write_count: Array.isArray(payload.semantic?.writes) ? payload.semantic.writes.length : null,
    related_test_count: Array.isArray(payload.related_tests) ? payload.related_tests.length : null,
  };
}

function summarizeParity(payload) {
  return {
    contract_count: payload.contract_count ?? null,
    missing_cell_count: payload.missing_cell_count ?? null,
    parity_score_0_10000: payload.parity_score_0_10000 ?? null,
    finding_count: Array.isArray(payload.findings) ? payload.findings.length : null,
  };
}

function summarizeState(payload) {
  return {
    state_model_count: payload.state_model_count ?? null,
    finding_count: payload.finding_count ?? null,
    state_integrity_score_0_10000: payload.state_integrity_score_0_10000 ?? null,
  };
}

function parseToolPayload(response) {
  if (response.result?.isError) {
    const message = response.result.content?.[0]?.text ?? 'Unknown MCP tool error';
    throw new Error(message);
  }

  const text = response.result?.content?.[0]?.text;
  if (typeof text !== 'string') {
    throw new Error('Missing MCP text payload');
  }

  return JSON.parse(text);
}

function createSession(binPath) {
  const child = spawn(binPath, ['--mcp'], {
    cwd: repoRoot,
    stdio: ['pipe', 'pipe', 'pipe'],
  });
  const pending = new Map();
  const stdoutLog = [];
  const stderrLog = [];
  let nextId = 1;
  let closed = false;

  const stdoutReader = readline.createInterface({ input: child.stdout });
  stdoutReader.on('line', (line) => {
    const trimmed = line.trim();
    if (!trimmed) {
      return;
    }

    if (!trimmed.startsWith('{')) {
      stdoutLog.push(trimmed);
      return;
    }

    let payload;
    try {
      payload = JSON.parse(trimmed);
    } catch (error) {
      stdoutLog.push(`unparsed-json:${trimmed}`);
      return;
    }

    const entry = pending.get(payload.id);
    if (!entry) {
      stdoutLog.push(`orphan-response:${trimmed}`);
      return;
    }

    clearTimeout(entry.timeout);
    pending.delete(payload.id);
    entry.resolve(payload);
  });

  const stderrReader = readline.createInterface({ input: child.stderr });
  stderrReader.on('line', (line) => {
    const trimmed = line.trim();
    if (trimmed) {
      stderrLog.push(trimmed);
    }
  });

  child.on('exit', (code, signal) => {
    closed = true;
    for (const entry of pending.values()) {
      clearTimeout(entry.timeout);
      entry.reject(new Error(`MCP process exited before response (code=${code}, signal=${signal})`));
    }
    pending.clear();
  });

  function call(name, args = {}) {
    if (closed) {
      throw new Error('MCP session is already closed');
    }

    const id = nextId;
    nextId += 1;
    const payload = {
      jsonrpc: '2.0',
      id,
      method: 'tools/call',
      params: {
        name,
        arguments: args,
      },
    };

    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        pending.delete(id);
        reject(new Error(`Timed out waiting for MCP response to ${name}`));
      }, requestTimeoutMs);

      pending.set(id, { resolve, reject, timeout });
      child.stdin.write(`${JSON.stringify(payload)}\n`, (error) => {
        if (!error) {
          return;
        }

        clearTimeout(timeout);
        pending.delete(id);
        reject(error);
      });
    });
  }

  async function close() {
    if (closed) {
      return;
    }

    child.stdin.end();
    await new Promise((resolve) => {
      child.once('exit', resolve);
    });
  }

  return {
    call,
    close,
    stdoutLog,
    stderrLog,
  };
}

async function measureRequest(session, label, name, args, summarize) {
  const startedAt = nowMs();
  const response = await session.call(name, args);
  const elapsedMs = roundMs(nowMs() - startedAt);
  const payload = parseToolPayload(response);

  return {
    label,
    tool: name,
    elapsed_ms: elapsedMs,
    summary: summarize(payload),
  };
}

async function runBenchmarkSession() {
  const session = createSession(sentruxBin);
  const cold = {};
  const warm = {};
  const coldStartedAt = nowMs();

  try {
    cold.scan = await measureRequest(
      session,
      'scan',
      'scan',
      { path: parallelCodeRoot },
      summarizeScan,
    );
    cold.concepts = await measureRequest(
      session,
      'concepts',
      'concepts',
      {},
      summarizeConcepts,
    );
    cold.findings = await measureRequest(
      session,
      'findings_top12',
      'findings',
      { limit: 12 },
      summarizeFindings,
    );
    cold.explain_task_git_status = await measureRequest(
      session,
      'explain_task_git_status',
      'explain_concept',
      { id: 'task_git_status' },
      summarizeExplainConcept,
    );
    cold.explain_task_presentation_status = await measureRequest(
      session,
      'explain_task_presentation_status',
      'explain_concept',
      { id: 'task_presentation_status' },
      summarizeExplainConcept,
    );
    cold.parity_server_state_bootstrap = await measureRequest(
      session,
      'parity_server_state_bootstrap',
      'parity',
      { contract: 'server_state_bootstrap' },
      summarizeParity,
    );
    cold.state = await measureRequest(
      session,
      'state',
      'state',
      {},
      summarizeState,
    );
    const coldProcessTotalMs = roundMs(nowMs() - coldStartedAt);

    const warmStartedAt = nowMs();
    warm.concepts = await measureRequest(
      session,
      'concepts',
      'concepts',
      {},
      summarizeConcepts,
    );
    warm.findings = await measureRequest(
      session,
      'findings_top12',
      'findings',
      { limit: 12 },
      summarizeFindings,
    );
    warm.explain_task_git_status = await measureRequest(
      session,
      'explain_task_git_status',
      'explain_concept',
      { id: 'task_git_status' },
      summarizeExplainConcept,
    );
    warm.explain_task_presentation_status = await measureRequest(
      session,
      'explain_task_presentation_status',
      'explain_concept',
      { id: 'task_presentation_status' },
      summarizeExplainConcept,
    );
    warm.parity_server_state_bootstrap = await measureRequest(
      session,
      'parity_server_state_bootstrap',
      'parity',
      { contract: 'server_state_bootstrap' },
      summarizeParity,
    );
    warm.state = await measureRequest(
      session,
      'state',
      'state',
      {},
      summarizeState,
    );
    const warmCachedTotalMs = roundMs(nowMs() - warmStartedAt);

    return {
      cold_process_total_ms: coldProcessTotalMs,
      cold,
      warm_cached_total_ms: warmCachedTotalMs,
      warm_cached: warm,
      stdout_log: session.stdoutLog,
      stderr_log: session.stderrLog,
    };
  } finally {
    await session.close();
  }
}

async function withInstalledRules(run) {
  await mkdir(parallelSentruxDir, { recursive: true });
  const rulesPreviouslyExisted = existsSync(parallelRulesPath);

  if (rulesPreviouslyExisted) {
    await cp(parallelRulesPath, parallelRulesBackupPath);
  }

  await cp(rulesSource, parallelRulesPath);

  try {
    return await run();
  } finally {
    if (existsSync(parallelRulesBackupPath)) {
      await rename(parallelRulesBackupPath, parallelRulesPath);
    } else if (existsSync(parallelRulesPath)) {
      await unlink(parallelRulesPath);
    }
  }
}

async function main() {
  assertPathExists(sentruxBin, 'sentrux binary');
  assertPathExists(rulesSource, 'parallel-code rules source');
  assertPathExists(parallelCodeRoot, 'parallel-code repo');

  const benchmark = await withInstalledRules(runBenchmarkSession);
  const result = {
    generated_at: new Date().toISOString(),
    parallel_code_root: parallelCodeRoot,
    sentrux_binary: sentruxBin,
    benchmark,
  };

  await mkdir(path.dirname(outputPath), { recursive: true });
  await writeFile(outputPath, `${JSON.stringify(result, null, 2)}\n`, 'utf8');
  console.log(`Wrote benchmark results to ${outputPath}`);
}

main().catch(async (error) => {
  console.error(error instanceof Error ? error.message : String(error));
  try {
    if (existsSync(parallelRulesBackupPath)) {
      await rename(parallelRulesBackupPath, parallelRulesPath);
    }
  } catch {}
  process.exitCode = 1;
});
