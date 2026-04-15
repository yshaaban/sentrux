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
} from './lib/benchmark-harness.mjs';
import {
  summarizeAgentBrief,
  summarizeCheck,
  summarizeConcepts,
  summarizeExplainConcept,
  summarizeFindings,
  summarizeGate,
  summarizeParity,
  summarizeScan,
  summarizeSessionEnd,
  summarizeSessionSave,
  summarizeState,
} from './lib/benchmark-summaries.mjs';
import { prepareTypeScriptBenchmarkHome } from './lib/benchmark-plugin-home.mjs';
import { assertPathExists, createDisposableRepoClone } from './lib/disposable-repo.mjs';
import { buildRepoFreshnessMetadata, resolveHeadCommitEpoch } from './lib/repo-identity.mjs';
import { resolveWorkspaceRepoRoot } from './lib/path-roots.mjs';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, '..');
const parallelCodeRoot = resolveWorkspaceRepoRoot(
  process.env.PARALLEL_CODE_ROOT,
  'parallel-code',
  repoRoot,
);
const sentruxBin = process.env.SENTRUX_BIN ?? path.join(repoRoot, 'target/debug/sentrux');
const rulesSource = path.join(repoRoot, 'docs/v2/examples/parallel-code.rules.toml');
const outputPath =
  process.env.OUTPUT_PATH ?? path.join(repoRoot, 'docs/v2/examples/parallel-code-benchmark.json');
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
  ['cold.concepts.elapsed_ms', 'cold concepts'],
  ['cold.agent_brief_onboarding.elapsed_ms', 'cold agent_brief onboarding'],
  ['warm_cached_total_ms', 'warm cached total'],
  ['warm_cached.findings.elapsed_ms', 'warm findings'],
  ['warm_cached.agent_brief_onboarding.elapsed_ms', 'warm agent_brief onboarding'],
  ['warm_persisted_total_ms', 'warm persisted total'],
  ['warm_persisted.concepts.elapsed_ms', 'warm persisted concepts'],
  ['warm_persisted.findings.elapsed_ms', 'warm persisted findings'],
  ['warm_persisted.agent_brief_onboarding.elapsed_ms', 'warm persisted agent_brief onboarding'],
  ['warm_patch_safety_total_ms', 'warm patch-safety total'],
  ['warm_patch_safety.session_start.elapsed_ms', 'warm session_start'],
  ['warm_patch_safety.agent_brief_patch.elapsed_ms', 'warm agent_brief patch'],
  ['warm_patch_safety.gate.elapsed_ms', 'warm gate'],
  ['warm_patch_safety.check.elapsed_ms', 'warm check'],
  ['warm_patch_safety.agent_brief_pre_merge.elapsed_ms', 'warm agent_brief pre_merge'],
  ['warm_patch_safety.session_end.elapsed_ms', 'warm session_end'],
];
const publicPathReplacements = [
  [compareToPath, '<sentrux-root>/docs/v2/examples/parallel-code-benchmark.json'],
  [sentruxBin, '<sentrux-root>/target/debug/sentrux'],
  [rulesSource, '<sentrux-root>/docs/v2/examples/parallel-code.rules.toml'],
  [parallelCodeRoot, '<parallel-code-root>'],
  [repoRoot, '<sentrux-root>'],
];

function sanitizePublicArtifactValue(value) {
  if (typeof value === 'string') {
    return publicPathReplacements.reduce(function replacePath(current, [target, replacement]) {
      return current.split(target).join(replacement);
    }, value);
  }

  if (Array.isArray(value)) {
    return value.map(sanitizePublicArtifactValue);
  }

  if (value && typeof value === 'object') {
    return Object.fromEntries(
      Object.entries(value).map(function sanitizeEntry([key, entry]) {
        return [key, sanitizePublicArtifactValue(entry)];
      }),
    );
  }

  return value;
}

function buildSessionEnv(fixedNowEpoch) {
  if (fixedNowEpoch == null) {
    return {};
  }

  return {
    SENTRUX_FIXED_NOW_EPOCH: String(fixedNowEpoch),
  };
}

function resolveFreshnessRepoRoot(frozenSourceRoot) {
  if (analysisMode === 'head_clone') {
    return frozenSourceRoot;
  }

  return parallelCodeRoot;
}

function createSession(homeOverride, fixedNowEpoch) {
  return createMcpSession({
    binPath: sentruxBin,
    repoRoot,
    homeOverride,
    skipGrammarDownload,
    requestTimeoutMs,
    extraEnv: buildSessionEnv(fixedNowEpoch),
  });
}

async function runBenchmarkSession(parallelCodeWorkRoot, homeOverride, fixedNowEpoch) {
  const session = createSession(homeOverride, fixedNowEpoch);
  let persistedSession = null;
  const cold = {};
  const warmCached = {};
  const warmPersisted = {};
  const warmPatchSafety = {};
  const coldStartedAt = nowMs();

  try {
    cold.scan = await runBenchmarkTool(
      session,
      'scan',
      'scan',
      { path: parallelCodeWorkRoot },
      summarizeScan,
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
    cold.explain_task_git_status = await runBenchmarkTool(
      session,
      'explain_task_git_status',
      'explain_concept',
      { id: 'task_git_status' },
      summarizeExplainConcept,
    );
    cold.explain_task_presentation_status = await runBenchmarkTool(
      session,
      'explain_task_presentation_status',
      'explain_concept',
      { id: 'task_presentation_status' },
      summarizeExplainConcept,
    );
    cold.parity_server_state_bootstrap = await runBenchmarkTool(
      session,
      'parity_server_state_bootstrap',
      'parity',
      { contract: 'server_state_bootstrap' },
      summarizeParity,
    );
    cold.state = await runBenchmarkTool(
      session,
      'state',
      'state',
      {},
      summarizeState,
    );
    cold.agent_brief_onboarding = await runBenchmarkTool(
      session,
      'agent_brief_onboarding',
      'agent_brief',
      { mode: 'repo_onboarding', limit: 3 },
      summarizeAgentBrief,
    );
    const coldProcessTotalMs = roundMs(nowMs() - coldStartedAt);

    const warmStartedAt = nowMs();
    warmCached.concepts = await runBenchmarkTool(
      session,
      'concepts',
      'concepts',
      {},
      summarizeConcepts,
    );
    warmCached.findings = await runBenchmarkTool(
      session,
      'findings_top12',
      'findings',
      { limit: 12 },
      summarizeFindings,
    );
    warmCached.explain_task_git_status = await runBenchmarkTool(
      session,
      'explain_task_git_status',
      'explain_concept',
      { id: 'task_git_status' },
      summarizeExplainConcept,
    );
    warmCached.explain_task_presentation_status = await runBenchmarkTool(
      session,
      'explain_task_presentation_status',
      'explain_concept',
      { id: 'task_presentation_status' },
      summarizeExplainConcept,
    );
    warmCached.parity_server_state_bootstrap = await runBenchmarkTool(
      session,
      'parity_server_state_bootstrap',
      'parity',
      { contract: 'server_state_bootstrap' },
      summarizeParity,
    );
    warmCached.state = await runBenchmarkTool(
      session,
      'state',
      'state',
      {},
      summarizeState,
    );
    warmCached.agent_brief_onboarding = await runBenchmarkTool(
      session,
      'agent_brief_onboarding',
      'agent_brief',
      { mode: 'repo_onboarding', limit: 3 },
      summarizeAgentBrief,
    );
    const warmCachedTotalMs = roundMs(nowMs() - warmStartedAt);

    const patchSafetyStartedAt = nowMs();
    warmPatchSafety.session_start = await runBenchmarkTool(
      session,
      'session_start',
      'session_start',
      {},
      summarizeSessionSave,
    );
    warmPatchSafety.agent_brief_patch = await runBenchmarkTool(
      session,
      'agent_brief_patch',
      'agent_brief',
      { mode: 'patch', limit: 3 },
      summarizeAgentBrief,
    );
    warmPatchSafety.gate = await runBenchmarkTool(
      session,
      'gate',
      'gate',
      {},
      summarizeGate,
    );
    warmPatchSafety.check = await runBenchmarkTool(
      session,
      'check',
      'check',
      {},
      summarizeCheck,
    );
    warmPatchSafety.agent_brief_pre_merge = await runBenchmarkTool(
      session,
      'agent_brief_pre_merge',
      'agent_brief',
      { mode: 'pre_merge', limit: 3 },
      summarizeAgentBrief,
    );
    warmPatchSafety.session_end = await runBenchmarkTool(
      session,
      'session_end',
      'session_end',
      {},
      summarizeSessionEnd,
    );
    const warmPatchSafetyTotalMs = roundMs(nowMs() - patchSafetyStartedAt);

    const warmPersistedStartedAt = nowMs();
    persistedSession = createSession(homeOverride, fixedNowEpoch);
    warmPersisted.scan = await runBenchmarkTool(
      persistedSession,
      'persisted_scan',
      'scan',
      { path: parallelCodeWorkRoot },
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

async function runBenchmarkSample(sampleIndex, frozenSourceRoot, freshnessMetadata) {
  const clone = await createDisposableRepoClone({
    sourceRoot: frozenSourceRoot,
    label: 'parallel-code-benchmark-sample',
    rulesSource,
    analysisMode: 'working_tree',
  });

  let benchmark;
  try {
    const fixedNowEpoch = resolveHeadCommitEpoch(clone.workRoot);
    const pluginHome = await prepareTypeScriptBenchmarkHome({ tempRoot: clone.tempRoot });
    benchmark = await runBenchmarkSession(clone.workRoot, pluginHome, fixedNowEpoch);
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
  assertPathExists(rulesSource, 'parallel-code rules source');
  assertPathExists(parallelCodeRoot, 'parallel-code repo');

  const previousResult = await loadPreviousBenchmark(compareToPath);
  const frozenSource = await createDisposableRepoClone({
    sourceRoot: parallelCodeRoot,
    label: 'parallel-code-benchmark-source',
    rulesSource,
    analysisMode,
  });
  let samples;
  let freshnessMetadata;
  try {
    freshnessMetadata = buildRepoFreshnessMetadata({
      repoRoot: resolveFreshnessRepoRoot(frozenSource.workRoot),
      analyzedRoot: frozenSource.workRoot,
      analysisMode,
      rulesSource,
      binaryPath: sentruxBin,
    });
    freshnessMetadata.parallel_code_root = parallelCodeRoot;
    ({ samples } = await runRepeatedBenchmarkSamples({
      repeatCount: benchmarkRepeatCount,
      runSample: function runFrozenSample(sampleIndex) {
        return runBenchmarkSample(sampleIndex, frozenSource.workRoot, freshnessMetadata);
      },
    }));
  } finally {
    await frozenSource.cleanup();
  }
  const aggregate = buildAggregatedBenchmark({ samples });
  if (!aggregate) {
    throw new Error('Failed to build aggregated benchmark samples');
  }

  const result = {
    benchmark_format_version: benchmarkFormatVersion,
    generated_at: new Date().toISOString(),
    parallel_code_root: parallelCodeRoot,
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
  await writeFile(
    outputPath,
    `${JSON.stringify(sanitizePublicArtifactValue(result), null, 2)}\n`,
    'utf8',
  );
  console.log(
    `Wrote benchmark results to ${outputPath} using ${aggregate.sample_count} sample(s) with median aggregation.`,
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
