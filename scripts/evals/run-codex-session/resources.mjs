import { writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { createDisposableRepoClone } from '../../lib/disposable-repo.mjs';
import { prepareTypeScriptBenchmarkHome } from '../../lib/benchmark-plugin-home.mjs';
import {
  createEvalMcpSession,
  defaultRulesSource,
  maybeCopyFile,
} from '../../lib/eval-support.mjs';
import {
  loadSessionTelemetrySummaryOrEmpty,
  summarizeOutcome,
  writeSessionTelemetryArtifacts,
} from '../../lib/session-telemetry.mjs';
import { slugify } from '../../lib/eval-batch.mjs';
import { nowIso } from './status.mjs';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, '../../..');
const sentruxBin = process.env.SENTRUX_BIN ?? path.join(repoRoot, 'target/debug/sentrux');

export async function createCodexRunResources({
  args,
  sourceRoot,
  repoLabel,
  taskLabel,
}) {
  const rulesSource =
    args.rulesSource == null ? defaultRulesSource(sourceRoot) : path.resolve(args.rulesSource);
  const clone = await createDisposableRepoClone({
    sourceRoot,
    label: `codex-session-${slugify(repoLabel)}-${slugify(taskLabel)}`,
    rulesSource,
    analysisMode: args.analysisMode,
  });
  const pluginHome = await prepareTypeScriptBenchmarkHome({ tempRoot: clone.tempRoot });
  const session = createEvalMcpSession({
    repoRoot,
    binPath: sentruxBin,
    homeOverride: pluginHome,
  });

  return { clone, session };
}

export async function loadCodexSessionTelemetry(
  sourceRoot,
  clone,
  copiedTelemetryLogPath,
) {
  const telemetryLogPath = path.join(clone.workRoot, '.sentrux', 'agent-session-events.jsonl');
  await maybeCopyFile(telemetryLogPath, copiedTelemetryLogPath);
  return loadSessionTelemetrySummaryOrEmpty(telemetryLogPath, {
    repoRoot: sourceRoot,
  });
}

export async function writeCodexRunArtifacts(paths, bundle, sessionTelemetry) {
  await writeFile(paths.bundlePath, `${JSON.stringify(bundle, null, 2)}\n`, 'utf8');
  await writeSessionTelemetryArtifacts({
    telemetryJsonPath: paths.telemetryJsonPath,
    telemetryMarkdownPath: paths.telemetryMarkdownPath,
    summary: sessionTelemetry,
  });
}

export function buildCodexBundle({
  args,
  repoLabel,
  taskId,
  sourceRoot,
  clone,
  taskLabel,
  paths,
  startedAt,
  providerRun,
  executionStatus,
  snapshots,
  finalSnapshot,
  finalGate,
  sessionEnd,
  sessionTelemetry,
}) {
  return {
    schema_version: 1,
    generated_at: nowIso(),
    repo_label: repoLabel,
    task_id: taskId,
    source_root: sourceRoot,
    analyzed_root: clone.workRoot,
    analysis_mode: args.analysisMode,
    task_label: taskLabel,
    tags: args.tags,
    expected_signal_kinds: args.expectedSignalKinds,
    expected_fix_surface: args.expectedFixSurface ?? null,
    prompt_path: paths.promptPath,
    started_at: startedAt,
    finished_at: nowIso(),
    provider_run: providerRun,
    status: executionStatus,
    provider_timeout_phase: providerRun.timeout_phase,
    initial_check: snapshots[0].check,
    snapshots,
    final_check: finalSnapshot.check,
    final_gate: finalGate.payload,
    session_end: sessionEnd.payload,
    telemetry_summary: sessionTelemetry,
    outcome: summarizeOutcome(sessionTelemetry),
  };
}
