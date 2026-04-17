import { collectRepoIdentity } from '../../lib/repo-identity.mjs';
import { startCodexExec } from '../providers/codex-cli.mjs';
import {
  buildRunningProviderStatus,
  buildSnapshotStatusFields,
  nowMs,
  sleep,
  summarizeTimeoutPhase,
} from './status.mjs';
import { recordSnapshot, shouldCaptureSnapshot } from './snapshots.mjs';

export async function monitorCodexProvider({
  args,
  clone,
  session,
  prompt,
  updateStatus,
}) {
  const snapshots = [await recordSnapshot(session, clone.workRoot, 'initial')];
  let latestSnapshot = snapshots[0];
  const running = await startCodexExec({
    cwd: clone.workRoot,
    prompt,
    model: args.model,
    timeoutMs: args.timeoutMs,
    codexBin: args.codexBin,
  });
  let idleTimeoutTriggered = false;
  let lastProgressAtMs = nowMs();
  let lastStdoutLength = running.stdoutLength;
  let lastStderrLength = running.stderrLength;

  await updateStatus('provider_running', {
    analyzed_root: clone.workRoot,
    snapshot_count: snapshots.length,
    ...buildRunningProviderStatus(running),
  });

  while (!running.finished) {
    await sleep(args.pollMs);
    const providerOutputChanged =
      running.stdoutLength !== lastStdoutLength || running.stderrLength !== lastStderrLength;
    if (providerOutputChanged) {
      lastStdoutLength = running.stdoutLength;
      lastStderrLength = running.stderrLength;
      lastProgressAtMs = nowMs();
      await updateStatus('provider_running', {
        analyzed_root: clone.workRoot,
        snapshot_count: snapshots.length,
        ...buildSnapshotStatusFields(latestSnapshot),
        ...buildRunningProviderStatus(running),
      });
    }

    const identity = collectRepoIdentity(clone.workRoot);
    if (shouldCaptureSnapshot(latestSnapshot, identity)) {
      const snapshot = await recordSnapshot(
        session,
        clone.workRoot,
        `poll-${snapshots.length}`,
      );
      snapshots.push(snapshot);
      latestSnapshot = snapshot;
      lastProgressAtMs = nowMs();
      await updateStatus('provider_running', {
        analyzed_root: clone.workRoot,
        snapshot_count: snapshots.length,
        ...buildSnapshotStatusFields(snapshot),
        ...buildRunningProviderStatus(running),
      });
    }

    if (
      args.idleTimeoutMs > 0 &&
      nowMs() - Math.max(lastProgressAtMs, running.lastOutputAtMs ?? 0) > args.idleTimeoutMs
    ) {
      idleTimeoutTriggered = true;
      await updateStatus('provider_idle_timeout', {
        analyzed_root: clone.workRoot,
        snapshot_count: snapshots.length,
        idle_timeout_ms: args.idleTimeoutMs,
        provider_timeout_phase: 'idle',
        ...buildRunningProviderStatus(running),
      });
      running.kill('SIGKILL');
    }
  }

  const providerRun = await running.wait();
  providerRun.idle_timed_out = idleTimeoutTriggered;
  providerRun.timeout_phase = summarizeTimeoutPhase(providerRun, idleTimeoutTriggered);

  return { snapshots, providerRun, idleTimeoutTriggered };
}
