#!/usr/bin/env node

import { spawn } from 'node:child_process';
import { existsSync } from 'node:fs';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import readline from 'node:readline';
import { fileURLToPath } from 'node:url';
import { assertPathExists, createDisposableRepoClone } from './lib/disposable-repo.mjs';
import { buildRepoFreshnessMetadata } from './lib/repo-identity.mjs';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, '..');
const parallelCodeRoot = process.env.PARALLEL_CODE_ROOT ?? '<parallel-code-root>';
const sentruxBin = process.env.SENTRUX_BIN ?? path.join(repoRoot, 'target/debug/sentrux');
const rulesSource = path.join(repoRoot, 'docs/v2/examples/parallel-code.rules.toml');
const outputPath =
  process.env.OUTPUT_PATH ?? path.join(repoRoot, 'docs/v2/examples/parallel-code-benchmark.json');
const compareToPath = process.env.COMPARE_TO ?? outputPath;
const requestTimeoutMs = Number(process.env.REQUEST_TIMEOUT_MS ?? '120000');
const analysisMode = process.env.ANALYSIS_MODE ?? 'working_tree';
const benchmarkPolicy = buildBenchmarkPolicy();
const failOnRegression = process.env.FAIL_ON_REGRESSION === '1';
const benchmarkFormatVersion = 3;

function roundMs(value) {
  return Number(value.toFixed(1));
}

function nowMs() {
  return Number(process.hrtime.bigint()) / 1_000_000;
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

function summarizeGate(payload) {
  return {
    decision: payload.decision ?? null,
    changed_file_count: Array.isArray(payload.changed_files) ? payload.changed_files.length : null,
    introduced_finding_count: Array.isArray(payload.introduced_findings)
      ? payload.introduced_findings.length
      : null,
    missing_obligation_count: Array.isArray(payload.missing_obligations)
      ? payload.missing_obligations.length
      : null,
    obligation_completeness_0_10000: payload.obligation_completeness_0_10000 ?? null,
  };
}

function summarizeSessionSave(payload) {
  return {
    session_finding_count: payload.session_finding_count ?? null,
    suppressed_finding_count: payload.suppressed_finding_count ?? null,
  };
}

function summarizeSessionEnd(payload) {
  return {
    pass: payload.pass ?? null,
    changed_file_count: Array.isArray(payload.changed_files) ? payload.changed_files.length : null,
    introduced_finding_count: Array.isArray(payload.introduced_findings)
      ? payload.introduced_findings.length
      : null,
    missing_obligation_count: Array.isArray(payload.missing_obligations)
      ? payload.missing_obligations.length
      : null,
    gate_decision: payload.touched_concept_gate?.decision ?? null,
  };
}

function getBenchmarkMetric(benchmark, metricPath) {
  return metricPath.split('.').reduce((value, key) => value?.[key], benchmark);
}

function safePercent(delta, baseline) {
  if (!Number.isFinite(delta) || !Number.isFinite(baseline) || baseline === 0) {
    return null;
  }
  return Number(((delta / baseline) * 100).toFixed(1));
}

function readNonNegativeNumber(value, fallback) {
  const parsed = Number(value);
  if (!Number.isFinite(parsed) || parsed < 0) {
    return fallback;
  }

  return parsed;
}

function buildBenchmarkPolicy() {
  const fail = {
    max_regression_ms: readNonNegativeNumber(process.env.MAX_REGRESSION_MS, 250),
    max_regression_percent: readNonNegativeNumber(process.env.MAX_REGRESSION_PERCENT, 20),
  };
  const warn = {
    max_regression_ms: Math.min(
      readNonNegativeNumber(process.env.WARN_REGRESSION_MS, 150),
      fail.max_regression_ms,
    ),
    max_regression_percent: Math.min(
      readNonNegativeNumber(process.env.WARN_REGRESSION_PERCENT, 10),
      fail.max_regression_percent,
    ),
  };

  return {
    fail,
    warn,
  };
}

function classifyBenchmarkMetric(deltaMs, deltaPercent) {
  const exceedsFail =
    deltaMs > benchmarkPolicy.fail.max_regression_ms &&
    deltaPercent !== null &&
    deltaPercent > benchmarkPolicy.fail.max_regression_percent;
  if (exceedsFail) {
    return 'fail';
  }

  const exceedsWarn =
    deltaMs > benchmarkPolicy.warn.max_regression_ms &&
    deltaPercent !== null &&
    deltaPercent > benchmarkPolicy.warn.max_regression_percent;
  if (exceedsWarn) {
    return 'warn';
  }

  return 'info';
}

function summarizeComparisonMetrics(metrics) {
  return metrics.reduce(
    (summary, metric) => {
      summary.total += 1;
      summary[`${metric.classification}_count`] += 1;
      return summary;
    },
    {
      total: 0,
      fail_count: 0,
      warn_count: 0,
      info_count: 0,
    },
  );
}

function buildBenchmarkComparison(currentResult, previousResult) {
  if (
    !previousResult?.benchmark ||
    previousResult.benchmark_format_version !== currentResult.benchmark_format_version
  ) {
    return null;
  }

  const trackedMetrics = [
    ['cold_process_total_ms', 'cold process total'],
    ['cold.scan.elapsed_ms', 'cold scan'],
    ['cold.concepts.elapsed_ms', 'cold concepts'],
    ['warm_cached_total_ms', 'warm cached total'],
    ['warm_cached.findings.elapsed_ms', 'warm findings'],
    ['warm_patch_safety_total_ms', 'warm patch-safety total'],
    ['warm_patch_safety.session_start.elapsed_ms', 'warm session_start'],
    ['warm_patch_safety.gate.elapsed_ms', 'warm gate'],
    ['warm_patch_safety.session_end.elapsed_ms', 'warm session_end'],
  ];

  const metrics = trackedMetrics
    .map(([metricPath, label]) => {
      const previousValue = getBenchmarkMetric(previousResult.benchmark, metricPath);
      const currentValue = getBenchmarkMetric(currentResult.benchmark, metricPath);
      if (!Number.isFinite(previousValue) || !Number.isFinite(currentValue)) {
        return null;
      }

      const deltaMs = Number((currentValue - previousValue).toFixed(1));
      const deltaPercent = safePercent(deltaMs, previousValue);
      const classification = classifyBenchmarkMetric(deltaMs, deltaPercent);

      return {
        metric: label,
        path: metricPath,
        previous_ms: previousValue,
        current_ms: currentValue,
        delta_ms: deltaMs,
        delta_percent: deltaPercent,
        classification,
      };
    })
    .filter(Boolean);
  const summary = summarizeComparisonMetrics(metrics);

  return {
    compared_to: compareToPath,
    previous_generated_at: previousResult.generated_at ?? null,
    policy: {
      fail: benchmarkPolicy.fail,
      warn: benchmarkPolicy.warn,
      mode: 'delta_ms_and_percent',
    },
    metrics,
    summary,
    regressions: metrics.filter((metric) => metric.classification === 'fail'),
    warnings: metrics.filter((metric) => metric.classification === 'warn'),
  };
}

function printBenchmarkPolicy(policy) {
  console.log(
    `Benchmark policy: fail at >${policy.fail.max_regression_ms}ms and >${policy.fail.max_regression_percent}%; warn at >${policy.warn.max_regression_ms}ms and >${policy.warn.max_regression_percent}%`,
  );
}

function printComparisonMetrics(heading, severity, metrics) {
  if (!metrics.length) {
    return;
  }

  console.log(`\n${heading}`);
  for (const metric of metrics) {
    console.log(
      `- [${severity}] ${metric.metric}: ${metric.previous_ms}ms -> ${metric.current_ms}ms (${metric.delta_ms}ms, ${metric.delta_percent}%)`,
    );
  }
}

function printBenchmarkComparison(comparison) {
  if (!comparison) {
    return;
  }

  printBenchmarkPolicy(comparison.policy);
  printComparisonMetrics('Benchmark fail regressions detected:', 'fail', comparison.regressions);
  printComparisonMetrics('Benchmark warning regressions detected:', 'warn', comparison.warnings);

  if (comparison.regressions.length === 0 && comparison.warnings.length === 0) {
    console.log('No benchmark regressions detected.');
  }
}

async function loadPreviousBenchmark(comparePath) {
  if (!existsSync(comparePath)) {
    return null;
  }

  const raw = await readFile(comparePath, 'utf8');
  return JSON.parse(raw);
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

async function runBenchmarkSession(parallelCodeWorkRoot) {
  const session = createSession(sentruxBin);
  const cold = {};
  const warm = {};
  const warmPatchSafety = {};
  const coldStartedAt = nowMs();

  try {
    cold.scan = await measureRequest(
      session,
      'scan',
      'scan',
      { path: parallelCodeWorkRoot },
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

    const patchSafetyStartedAt = nowMs();
    warmPatchSafety.session_start = await measureRequest(
      session,
      'session_start',
      'session_start',
      {},
      summarizeSessionSave,
    );
    warmPatchSafety.gate = await measureRequest(
      session,
      'gate',
      'gate',
      {},
      summarizeGate,
    );
    warmPatchSafety.session_end = await measureRequest(
      session,
      'session_end',
      'session_end',
      {},
      summarizeSessionEnd,
    );
    const warmPatchSafetyTotalMs = roundMs(nowMs() - patchSafetyStartedAt);

    return {
      cold_process_total_ms: coldProcessTotalMs,
      cold,
      warm_cached_total_ms: warmCachedTotalMs,
      warm_cached: warm,
      warm_patch_safety_total_ms: warmPatchSafetyTotalMs,
      warm_patch_safety: warmPatchSafety,
      stdout_log: session.stdoutLog,
      stderr_log: session.stderrLog,
    };
  } finally {
    await session.close();
  }
}

async function main() {
  assertPathExists(sentruxBin, 'sentrux binary');
  assertPathExists(rulesSource, 'parallel-code rules source');
  assertPathExists(parallelCodeRoot, 'parallel-code repo');

  const previousResult = await loadPreviousBenchmark(compareToPath);
  const clone = await createDisposableRepoClone({
    sourceRoot: parallelCodeRoot,
    label: 'parallel-code-benchmark',
    rulesSource,
    analysisMode,
  });
  let benchmark;
  let freshnessMetadata;
  try {
    benchmark = await runBenchmarkSession(clone.workRoot);
    freshnessMetadata = buildRepoFreshnessMetadata({
      repoRoot: parallelCodeRoot,
      analyzedRoot: clone.workRoot,
      analysisMode,
      rulesSource,
      binaryPath: sentruxBin,
    });
  } finally {
    await clone.cleanup();
  }
  const result = {
    benchmark_format_version: benchmarkFormatVersion,
    generated_at: new Date().toISOString(),
    parallel_code_root: parallelCodeRoot,
    ...freshnessMetadata,
    sentrux_binary: sentruxBin,
    benchmark,
  };
  const comparison = buildBenchmarkComparison(result, previousResult);
  if (comparison) {
    result.comparison = comparison;
  }

  await mkdir(path.dirname(outputPath), { recursive: true });
  await writeFile(outputPath, `${JSON.stringify(result, null, 2)}\n`, 'utf8');
  console.log(`Wrote benchmark results to ${outputPath}`);

  printBenchmarkComparison(comparison);
  if (comparison?.regressions.length && failOnRegression) {
    process.exitCode = 1;
  }
}

main().catch(async (error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
});
