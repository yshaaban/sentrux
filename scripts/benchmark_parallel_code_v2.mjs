#!/usr/bin/env node

import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  buildBenchmarkComparison,
  buildBenchmarkPolicy,
  createMcpSession,
  loadPreviousBenchmark,
  nowMs,
  printBenchmarkComparison,
  roundMs,
  runBenchmarkTool,
} from './lib/benchmark-harness.mjs';
import {
  summarizeAgentBrief,
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
import { buildRepoFreshnessMetadata } from './lib/repo-identity.mjs';
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
const analysisMode = process.env.ANALYSIS_MODE ?? 'working_tree';
const benchmarkPolicy = buildBenchmarkPolicy();
const failOnRegression = process.env.FAIL_ON_REGRESSION === '1';
const benchmarkFormatVersion = 3;
const skipGrammarDownload = process.env.SENTRUX_SKIP_GRAMMAR_DOWNLOAD ?? '1';

function createSession(homeOverride) {
  return createMcpSession({
    binPath: sentruxBin,
    repoRoot,
    homeOverride,
    skipGrammarDownload,
    requestTimeoutMs,
  });
}

async function runBenchmarkSession(parallelCodeWorkRoot, homeOverride) {
  const session = createSession(homeOverride);
  let persistedSession = null;
  const cold = {};
  const warm = {};
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
    warm.concepts = await runBenchmarkTool(
      session,
      'concepts',
      'concepts',
      {},
      summarizeConcepts,
    );
    warm.findings = await runBenchmarkTool(
      session,
      'findings_top12',
      'findings',
      { limit: 12 },
      summarizeFindings,
    );
    warm.explain_task_git_status = await runBenchmarkTool(
      session,
      'explain_task_git_status',
      'explain_concept',
      { id: 'task_git_status' },
      summarizeExplainConcept,
    );
    warm.explain_task_presentation_status = await runBenchmarkTool(
      session,
      'explain_task_presentation_status',
      'explain_concept',
      { id: 'task_presentation_status' },
      summarizeExplainConcept,
    );
    warm.parity_server_state_bootstrap = await runBenchmarkTool(
      session,
      'parity_server_state_bootstrap',
      'parity',
      { contract: 'server_state_bootstrap' },
      summarizeParity,
    );
    warm.state = await runBenchmarkTool(
      session,
      'state',
      'state',
      {},
      summarizeState,
    );
    warm.agent_brief_onboarding = await runBenchmarkTool(
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
    persistedSession = createSession(homeOverride);
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
      warm_cached: warm,
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
    const pluginHome = await prepareTypeScriptBenchmarkHome({ tempRoot: clone.tempRoot });
    benchmark = await runBenchmarkSession(clone.workRoot, pluginHome);
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
  const comparison = buildBenchmarkComparison({
    currentResult: result,
    previousResult,
    compareToPath,
    benchmarkPolicy,
    trackedMetrics: [
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
      ['warm_patch_safety.agent_brief_pre_merge.elapsed_ms', 'warm agent_brief pre_merge'],
      ['warm_patch_safety.session_end.elapsed_ms', 'warm session_end'],
    ],
  });
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
