import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';

import { runTool } from '../../lib/benchmark-harness.mjs';
import { slugify } from '../../lib/eval-batch.mjs';
import { loadPrompt, defaultOutputDir, resolveRepoLabel } from './args.mjs';
import { monitorCodexProvider } from './provider.mjs';
import {
  buildCodexBundle,
  createCodexRunResources,
  loadCodexSessionTelemetry,
  writeCodexRunArtifacts,
} from './resources.mjs';
import { recordSnapshot } from './snapshots.mjs';
import {
  buildCodexOutputPaths,
  buildCompletedProviderStatus,
  buildRunStatusBase,
  buildRunStatusPayload,
  nowIso,
  summarizeProviderStatus,
  writeRunStatus,
} from './status.mjs';
import { applyExperimentArmToPrompt } from './intervention-arms.mjs';

function buildCompletedCodexStatus(bundle, executionStatus, snapshots, providerRun, clone) {
  return {
    analyzed_root: clone.workRoot,
    finished_at: bundle.finished_at,
    status: executionStatus,
    snapshot_count: snapshots.length,
    final_gate: bundle.outcome.final_gate ?? null,
    final_session_clean: bundle.outcome.final_session_clean ?? false,
    ...buildCompletedProviderStatus(providerRun),
  };
}

export async function runCodexSession(options) {
  const args = {
    ...options,
    tags: [...(options.tags ?? [])],
    expectedSignalKinds: [...(options.expectedSignalKinds ?? [])],
  };
  const sourceRoot = path.resolve(args.sourceRoot);
  const repoLabel = resolveRepoLabel(sourceRoot, args.repoLabel);
  const rawPrompt = await loadPrompt(args);
  const taskLabel = args.taskLabel ?? rawPrompt.split(/\r?\n/, 1)[0] ?? 'codex-session';
  const prompt = applyExperimentArmToPrompt(rawPrompt, args);
  const taskId = args.taskId ?? slugify(taskLabel);
  const outputDir = path.resolve(args.outputDir ?? defaultOutputDir(sourceRoot, taskLabel));
  const paths = buildCodexOutputPaths(outputDir);
  const { clone, session } = await createCodexRunResources({
    args,
    sourceRoot,
    repoLabel,
    taskLabel,
  });
  const startedAt = nowIso();
  const statusBase = buildRunStatusBase(taskId, taskLabel, repoLabel, startedAt, outputDir);

  async function updateStatus(phase, extra = {}) {
    await writeRunStatus(paths.statusPath, buildRunStatusPayload(statusBase, phase, extra));
  }

  await mkdir(outputDir, { recursive: true });
  await writeFile(paths.promptPath, prompt, 'utf8');
  await updateStatus('initializing');

  try {
    console.log(`Starting Codex session for ${repoLabel}: ${taskLabel}`);
    await runTool(session, 'scan', { path: clone.workRoot });
    await runTool(session, 'session_start', {});
    await updateStatus('session_started', {
      analyzed_root: clone.workRoot,
    });

    const { snapshots, providerRun, idleTimeoutTriggered } = await monitorCodexProvider({
      args,
      clone,
      session,
      prompt,
      updateStatus,
    });
    const finalSnapshot = await recordSnapshot(session, clone.workRoot, 'final');
    const finalGate = await runTool(session, 'gate', {});
    const sessionEnd = await runTool(session, 'session_end', {});
    const sessionTelemetry = await loadCodexSessionTelemetry(
      sourceRoot,
      clone,
      paths.copiedTelemetryLogPath,
    );
    const executionStatus = summarizeProviderStatus(providerRun, idleTimeoutTriggered);
    const bundle = buildCodexBundle({
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
    });

    await writeCodexRunArtifacts(paths, bundle, sessionTelemetry);
    await updateStatus(
      'completed',
      buildCompletedCodexStatus(
        bundle,
        executionStatus,
        snapshots,
        providerRun,
        clone,
      ),
    );

    console.log(
      `Captured Codex session for ${repoLabel} with ${snapshots.length} check snapshot(s); status=${executionStatus}; final gate=${bundle.outcome.final_gate ?? 'unknown'}.`,
    );
    console.log(`Artifacts written to ${outputDir}`);
    return bundle;
  } finally {
    await session.close();
    if (!args.keepClone) {
      await clone.cleanup();
    }
  }
}
