#!/usr/bin/env node

import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  buildAggregatedBenchmark,
  buildBenchmarkComparison,
  buildBenchmarkPolicy,
  createMcpSession,
  loadPreviousBenchmark,
  nowMs,
  printBenchmarkComparison,
  readPositiveInteger,
  runRepeatedBenchmarkSamples,
  roundMs,
  runBenchmarkTool,
  runTool,
} from './lib/benchmark-harness.mjs';
import {
  summarizeAgentBrief,
  summarizeCheck,
  summarizeCheckRules,
  summarizeConcepts,
  summarizeFindings,
  summarizeGate,
  summarizeProjectShape,
  summarizeScan,
  summarizeSessionEnd,
  summarizeSessionSave,
} from './lib/benchmark-summaries.mjs';
import { prepareTypeScriptBenchmarkHome } from './lib/benchmark-plugin-home.mjs';
import { assertPathExists, createDisposableRepoClone } from './lib/disposable-repo.mjs';
import { buildRepoFreshnessMetadata } from './lib/repo-identity.mjs';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, '..');
const sentruxBin = process.env.SENTRUX_BIN ?? path.join(repoRoot, 'target/debug/sentrux');
const rulesSource = path.join(repoRoot, '.sentrux/rules.toml');
const outputPath =
  process.env.OUTPUT_PATH ?? path.join(repoRoot, 'docs/v2/examples/sentrux-benchmark.json');
const compareToPath = process.env.COMPARE_TO ?? outputPath;
const requestTimeoutMs = Number(process.env.REQUEST_TIMEOUT_MS ?? '120000');
const analysisMode = process.env.ANALYSIS_MODE ?? 'head_clone';
const benchmarkPolicy = buildBenchmarkPolicy();
const failOnRegression = process.env.FAIL_ON_REGRESSION === '1';
const failOnNonComparable = process.env.FAIL_ON_NONCOMPARABLE === '1';
const benchmarkRepeatCount = readPositiveInteger(process.env.BENCHMARK_REPEATS ?? '3', 3);
const benchmarkFormatVersion = 3;
const skipGrammarDownload = process.env.SENTRUX_SKIP_GRAMMAR_DOWNLOAD ?? '1';
const comparisonMetrics = [
  ['cold_process_total_ms', 'cold process total'],
  ['cold.scan.elapsed_ms', 'cold scan'],
  ['cold.project_shape.elapsed_ms', 'cold project_shape'],
  ['cold.concepts.elapsed_ms', 'cold concepts'],
  ['cold.findings.elapsed_ms', 'cold findings'],
  ['cold.check_rules.elapsed_ms', 'cold check_rules'],
  ['cold.agent_brief_onboarding.elapsed_ms', 'cold agent_brief onboarding'],
  ['warm_cached_total_ms', 'warm cached total'],
  ['warm_cached.findings.elapsed_ms', 'warm findings'],
  ['warm_cached.check_rules.elapsed_ms', 'warm check_rules'],
  ['warm_cached.agent_brief_onboarding.elapsed_ms', 'warm agent_brief onboarding'],
  ['warm_persisted_total_ms', 'warm persisted total'],
  ['warm_persisted.concepts.elapsed_ms', 'warm persisted concepts'],
  ['warm_persisted.findings.elapsed_ms', 'warm persisted findings'],
  ['warm_persisted.agent_brief_onboarding.elapsed_ms', 'warm persisted agent_brief onboarding'],
  ['warm_patch_safety_total_ms', 'warm patch-safety total'],
  ['warm_patch_safety.session_start.elapsed_ms', 'warm session_start'],
  ['warm_patch_safety.check.elapsed_ms', 'warm check'],
  ['warm_patch_safety.gate.elapsed_ms', 'warm gate'],
  ['warm_patch_safety.session_end.elapsed_ms', 'warm session_end'],
];

function createSession(homeOverride) {
  return createMcpSession({
    binPath: sentruxBin,
    repoRoot,
    homeOverride,
    skipGrammarDownload,
    requestTimeoutMs,
  });
}

async function runBenchmarkSession(workRoot, homeOverride) {
  const session = createSession(homeOverride);
  let persistedSession = null;
  const cold = {};
  const warmCached = {};
  const warmPersisted = {};
  const warmPatchSafety = {};
  const coldStartedAt = nowMs();

  try {
    cold.scan = await runBenchmarkTool(session, 'scan', 'scan', { path: workRoot }, summarizeScan);
    cold.project_shape = await runBenchmarkTool(
      session,
      'project_shape',
      'project_shape',
      {},
      summarizeProjectShape,
    );
    cold.concepts = await runBenchmarkTool(
      session,
      'concepts',
      'concepts',
      {},
      summarizeConcepts,
    );
    cold.findings = await runBenchmarkTool(
      session,
      'findings_top12',
      'findings',
      { limit: 12 },
      summarizeFindings,
    );
    cold.check_rules = await runBenchmarkTool(
      session,
      'check_rules',
      'check_rules',
      {},
      summarizeCheckRules,
    );
    cold.agent_brief_onboarding = await runBenchmarkTool(
      session,
      'agent_brief_onboarding',
      'agent_brief',
      { mode: 'repo_onboarding', limit: 3 },
      summarizeAgentBrief,
    );
    const coldProcessTotalMs = roundMs(nowMs() - coldStartedAt);

    await runTool(session, 'project_shape', {});
    await runTool(session, 'findings', { limit: 12 });
    await runTool(session, 'check_rules', {});
    await runTool(session, 'agent_brief', { mode: 'repo_onboarding', limit: 3 });

    const warmCachedStartedAt = nowMs();
    warmCached.project_shape = await runBenchmarkTool(
      session,
      'project_shape',
      'project_shape',
      {},
      summarizeProjectShape,
    );
    warmCached.findings = await runBenchmarkTool(
      session,
      'findings_top12',
      'findings',
      { limit: 12 },
      summarizeFindings,
    );
    warmCached.check_rules = await runBenchmarkTool(
      session,
      'check_rules',
      'check_rules',
      {},
      summarizeCheckRules,
    );
    warmCached.agent_brief_onboarding = await runBenchmarkTool(
      session,
      'agent_brief_onboarding',
      'agent_brief',
      { mode: 'repo_onboarding', limit: 3 },
      summarizeAgentBrief,
    );
    const warmCachedTotalMs = roundMs(nowMs() - warmCachedStartedAt);

    const warmPatchSafetyStartedAt = nowMs();
    warmPatchSafety.session_start = await runBenchmarkTool(
      session,
      'session_start',
      'session_start',
      {},
      summarizeSessionSave,
    );
    warmPatchSafety.check = await runBenchmarkTool(
      session,
      'check',
      'check',
      {},
      summarizeCheck,
    );
    warmPatchSafety.gate = await runBenchmarkTool(session, 'gate', 'gate', {}, summarizeGate);
    warmPatchSafety.session_end = await runBenchmarkTool(
      session,
      'session_end',
      'session_end',
      {},
      summarizeSessionEnd,
    );
    const warmPatchSafetyTotalMs = roundMs(nowMs() - warmPatchSafetyStartedAt);

    const warmPersistedStartedAt = nowMs();
    persistedSession = createSession(homeOverride);
    warmPersisted.scan = await runBenchmarkTool(
      persistedSession,
      'persisted_scan',
      'scan',
      { path: workRoot },
      summarizeScan,
    );
    warmPersisted.concepts = await runBenchmarkTool(
      persistedSession,
      'persisted_concepts',
      'concepts',
      {},
      summarizeConcepts,
    );
    warmPersisted.findings = await runBenchmarkTool(
      persistedSession,
      'persisted_findings_top12',
      'findings',
      { limit: 12 },
      summarizeFindings,
    );
    warmPersisted.agent_brief_onboarding = await runBenchmarkTool(
      persistedSession,
      'persisted_agent_brief_onboarding',
      'agent_brief',
      { mode: 'repo_onboarding', limit: 3 },
      summarizeAgentBrief,
    );
    const warmPersistedTotalMs = roundMs(nowMs() - warmPersistedStartedAt);
    await persistedSession.close();
    persistedSession = null;

    return {
      cold_process_total_ms: coldProcessTotalMs,
      cold,
      warm_cached_total_ms: warmCachedTotalMs,
      warm_cached: warmCached,
      warm_persisted_total_ms: warmPersistedTotalMs,
      warm_persisted: warmPersisted,
      warm_patch_safety_total_ms: warmPatchSafetyTotalMs,
      warm_patch_safety: warmPatchSafety,
      stdout_log: session.stdoutLog,
      stderr_log: session.stderrLog,
    };
  } finally {
    if (persistedSession) {
      await persistedSession.close();
    }
    await session.close();
  }
}

async function runBenchmarkSample(sampleIndex) {
  const clone = await createDisposableRepoClone({
    sourceRoot: repoRoot,
    label: 'sentrux-benchmark',
    rulesSource,
    analysisMode,
  });

  let benchmark;
  let freshnessMetadata;
  try {
    const { parallel_code_root: _ignored, ...metadata } = buildRepoFreshnessMetadata({
      repoRoot,
      analyzedRoot: clone.workRoot,
      analysisMode,
      rulesSource,
      binaryPath: sentruxBin,
    });
    freshnessMetadata = metadata;
    const pluginHome = await prepareTypeScriptBenchmarkHome({ tempRoot: clone.tempRoot });
    benchmark = await runBenchmarkSession(clone.workRoot, pluginHome);
  } finally {
    await clone.cleanup();
  }

  return {
    sample_id: `sample_${sampleIndex + 1}`,
    generated_at: new Date().toISOString(),
    benchmark,
    freshnessMetadata,
  };
}

async function main() {
  assertPathExists(sentruxBin, 'sentrux binary');
  assertPathExists(rulesSource, 'sentrux rules source');
  assertPathExists(repoRoot, 'sentrux repo');

  const previousResult = await loadPreviousBenchmark(compareToPath);
  const { samples, freshnessMetadata } = await runRepeatedBenchmarkSamples({
    repeatCount: benchmarkRepeatCount,
    runSample: runBenchmarkSample,
  });
  const aggregate = buildAggregatedBenchmark({ samples });
  if (!aggregate) {
    throw new Error('Failed to build aggregated benchmark samples');
  }

  const result = {
    benchmark_format_version: benchmarkFormatVersion,
    generated_at: new Date().toISOString(),
    repo: 'sentrux',
    repo_root: repoRoot,
    benchmark_repeat_count: aggregate.sample_count,
    benchmark_aggregate_basis: 'median',
    benchmark_representative_sample_index: aggregate.representative_sample_index,
    benchmark_representative_sample_id: aggregate.representative_sample_id,
    benchmark_metric_statistics: aggregate.metric_statistics,
    benchmark_samples: samples,
    ...freshnessMetadata,
    sentrux_binary: sentruxBin,
    benchmark: aggregate.aggregate_benchmark,
  };

  const comparison = buildBenchmarkComparison({
    currentResult: result,
    previousResult,
    compareToPath,
    benchmarkPolicy,
    trackedMetrics: comparisonMetrics,
  });
  if (comparison) {
    result.comparison = comparison;
  }

  await mkdir(path.dirname(outputPath), { recursive: true });
  await writeFile(outputPath, `${JSON.stringify(result, null, 2)}\n`, 'utf8');
  console.log(
    `Wrote Sentrux v2 benchmark to ${outputPath} using ${aggregate.sample_count} sample(s) with median aggregation.`,
  );

  printBenchmarkComparison(comparison);
  if (comparison?.regressions.length && failOnRegression) {
    process.exitCode = 1;
  }
  if (comparison && !comparison.comparable && failOnNonComparable) {
    process.exitCode = 1;
  }
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
});
