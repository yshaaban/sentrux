#!/usr/bin/env node

import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  buildBenchmarkComparison,
  buildBenchmarkPolicy,
  buildAggregatedBenchmark,
  loadPreviousBenchmark,
  printBenchmarkComparison,
  readPositiveInteger,
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
import {
  collectFrozenBenchmarkSamples,
  resolveFreshnessRepoRoot,
  runBenchmarkSessionPhases,
  sanitizePublicArtifactValue,
} from './lib/benchmark-script-support.mjs';
import { assertPathExists } from './lib/disposable-repo.mjs';
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
const publicPathReplacements = [
  [compareToPath, '<sentrux-root>/docs/v2/examples/sentrux-benchmark.json'],
  [sentruxBin, '<sentrux-root>/target/debug/sentrux'],
  [rulesSource, '<sentrux-root>/.sentrux/rules.toml'],
  [repoRoot, '<sentrux-root>'],
];

async function runBenchmarkSession(workRoot, homeOverride, fixedNowEpoch) {
  return runBenchmarkSessionPhases({
    binPath: sentruxBin,
    repoRoot,
    workRoot,
    homeOverride,
    skipGrammarDownload,
    requestTimeoutMs,
    fixedNowEpoch,
    coldOperations: [
      { key: 'scan', tool: 'scan', args: { path: workRoot }, summarize: summarizeScan },
      { key: 'project_shape', tool: 'project_shape', summarize: summarizeProjectShape },
      { key: 'concepts', tool: 'concepts', summarize: summarizeConcepts },
      { key: 'findings', label: 'findings_top12', tool: 'findings', args: { limit: 12 }, summarize: summarizeFindings },
      { key: 'check_rules', tool: 'check_rules', summarize: summarizeCheckRules },
      { key: 'agent_brief_onboarding', tool: 'agent_brief', args: { mode: 'repo_onboarding', limit: 3 }, summarize: summarizeAgentBrief },
    ],
    warmOperations: [
      { tool: 'project_shape' },
      { tool: 'findings', args: { limit: 12 } },
      { tool: 'check_rules' },
      { tool: 'agent_brief', args: { mode: 'repo_onboarding', limit: 3 } },
    ],
    warmCachedOperations: [
      { key: 'project_shape', tool: 'project_shape', summarize: summarizeProjectShape },
      { key: 'findings', label: 'findings_top12', tool: 'findings', args: { limit: 12 }, summarize: summarizeFindings },
      { key: 'check_rules', tool: 'check_rules', summarize: summarizeCheckRules },
      { key: 'agent_brief_onboarding', tool: 'agent_brief', args: { mode: 'repo_onboarding', limit: 3 }, summarize: summarizeAgentBrief },
    ],
    warmPatchSafetyOperations: [
      { key: 'session_start', tool: 'session_start', summarize: summarizeSessionSave },
      { key: 'check', tool: 'check', summarize: summarizeCheck },
      { key: 'gate', tool: 'gate', summarize: summarizeGate },
      { key: 'session_end', tool: 'session_end', summarize: summarizeSessionEnd },
    ],
    warmPersistedOperations: [
      { key: 'scan', label: 'persisted_scan', tool: 'scan', args: { path: workRoot }, summarize: summarizeScan },
      { key: 'concepts', label: 'persisted_concepts', tool: 'concepts', summarize: summarizeConcepts },
      { key: 'findings', label: 'persisted_findings_top12', tool: 'findings', args: { limit: 12 }, summarize: summarizeFindings },
      { key: 'agent_brief_onboarding', label: 'persisted_agent_brief_onboarding', tool: 'agent_brief', args: { mode: 'repo_onboarding', limit: 3 }, summarize: summarizeAgentBrief },
    ],
  });
}

async function main() {
  assertPathExists(sentruxBin, 'sentrux binary');
  assertPathExists(rulesSource, 'sentrux rules source');
  assertPathExists(repoRoot, 'sentrux repo');

  const previousResult = await loadPreviousBenchmark(compareToPath);
  const { freshnessMetadata, samples } = await collectFrozenBenchmarkSamples({
    sourceRoot: repoRoot,
    cloneLabel: 'sentrux-benchmark-source',
    rulesSource,
    analysisMode,
    repeatCount: benchmarkRepeatCount,
    buildFreshnessMetadata: function buildFreshness(frozenSourceRoot) {
      return buildRepoFreshnessMetadata({
        repoRoot: resolveFreshnessRepoRoot(analysisMode, frozenSourceRoot, repoRoot),
        analyzedRoot: frozenSourceRoot,
        analysisMode,
        rulesSource,
        binaryPath: sentruxBin,
      });
    },
    runBenchmarkSession,
    sampleLabel: 'sentrux-benchmark-sample',
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
  await writeFile(
    outputPath,
    `${JSON.stringify(sanitizePublicArtifactValue(result, publicPathReplacements), null, 2)}\n`,
    'utf8',
  );
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
