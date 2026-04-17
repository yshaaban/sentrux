import { writeFile } from 'node:fs/promises';
import path from 'node:path';
import { nowIso, nowMs } from '../../lib/eval-runtime/common.mjs';

export { nowIso, nowMs } from '../../lib/eval-runtime/common.mjs';

export function sleep(ms) {
  return new Promise((resolve) => {
    setTimeout(resolve, ms);
  });
}

export async function writeRunStatus(targetPath, status) {
  await writeFile(targetPath, `${JSON.stringify(status, null, 2)}\n`, 'utf8');
}

export function buildRunStatusBase(taskId, taskLabel, repoLabel, startedAt, outputDir) {
  return {
    task_id: taskId,
    task_label: taskLabel,
    repo_label: repoLabel,
    started_at: startedAt,
    output_dir: outputDir,
  };
}

export function buildRunStatusPayload(statusBase, phase, extra = {}) {
  return {
    phase,
    ...statusBase,
    ...extra,
  };
}

export function buildSnapshotStatusFields(snapshot) {
  return {
    latest_snapshot_label: snapshot?.label ?? null,
    latest_snapshot_gate: snapshot?.gate ?? null,
  };
}

export function summarizeProviderStatus(providerRun, idleTimeoutTriggered) {
  if (idleTimeoutTriggered) {
    return 'provider_idle_timeout';
  }
  if (providerRun?.timed_out) {
    return 'provider_timeout';
  }
  if ((providerRun?.exit_code ?? null) !== 0) {
    return 'provider_failed';
  }

  return 'completed';
}

export function summarizeTimeoutPhase(providerRun, idleTimeoutTriggered) {
  if (idleTimeoutTriggered || providerRun?.idle_timed_out) {
    return 'idle';
  }
  if (!providerRun?.timed_out) {
    return null;
  }

  const inProgressCommand = providerRun?.event_summary?.in_progress_command?.command ?? null;
  if (inProgressCommand) {
    return 'command_execution';
  }
  if ((providerRun?.event_summary?.turn_count ?? 0) > 0) {
    return 'model_turn';
  }
  if ((providerRun?.event_summary?.event_count ?? 0) > 0) {
    return 'provider_stream';
  }

  return 'startup';
}

export function buildRunningProviderStatus(running) {
  const eventSummary = running.eventSummary ?? null;
  return {
    provider_pid: running.pid ?? null,
    provider_event_count: eventSummary?.event_count ?? 0,
    provider_turn_count: eventSummary?.turn_count ?? 0,
    provider_last_event_type: eventSummary?.last_event_type ?? null,
    provider_last_completed_command:
      eventSummary?.last_completed_command?.command ?? null,
    provider_in_progress_command: eventSummary?.in_progress_command?.command ?? null,
  };
}

export function buildCompletedProviderStatus(providerRun) {
  const eventSummary = providerRun?.event_summary ?? null;
  return {
    provider_exit_code: providerRun?.exit_code ?? null,
    provider_timed_out: providerRun?.timed_out ?? false,
    provider_idle_timed_out: providerRun?.idle_timed_out ?? false,
    provider_timeout_phase: providerRun?.timeout_phase ?? null,
    provider_event_count: eventSummary?.event_count ?? 0,
    provider_turn_count: eventSummary?.turn_count ?? 0,
    provider_last_event_type: eventSummary?.last_event_type ?? null,
    provider_last_completed_command:
      eventSummary?.last_completed_command?.command ?? null,
    provider_last_completed_command_status:
      eventSummary?.last_completed_command_status ?? null,
    provider_in_progress_command: eventSummary?.in_progress_command?.command ?? null,
  };
}

export function buildCodexOutputPaths(outputDir) {
  return {
    promptPath: path.join(outputDir, 'task-prompt.txt'),
    statusPath: path.join(outputDir, 'run-status.json'),
    bundlePath: path.join(outputDir, 'codex-session.json'),
    telemetryJsonPath: path.join(outputDir, 'session-telemetry-summary.json'),
    telemetryMarkdownPath: path.join(outputDir, 'session-telemetry-summary.md'),
    copiedTelemetryLogPath: path.join(outputDir, 'agent-session-events.jsonl'),
  };
}
