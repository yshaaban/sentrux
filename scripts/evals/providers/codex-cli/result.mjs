import { parseJsonLine, parseJsonLines } from './json-lines.mjs';
import { resolveCodexVersion } from './shared.mjs';

export function buildCodexResult({
  codexBin,
  args,
  cwd,
  startedAt,
  durationMs,
  captured,
  lastMessage,
}) {
  const stdoutJsonl = parseJsonLines(captured.stdout);
  const lastMessageJson = parseJsonLine(lastMessage);

  return {
    provider: 'codex-cli',
    provider_version: resolveCodexVersion(codexBin),
    command: {
      executable: codexBin,
      args,
    },
    cwd,
    started_at: startedAt,
    duration_ms: durationMs,
    exit_code: captured.exitCode,
    signal: captured.signal,
    timed_out: captured.timedOut,
    stdout: captured.stdout,
    stderr: captured.stderr,
    stdout_jsonl: stdoutJsonl,
    stdout_json: lastMessageJson ?? stdoutJsonl.at(-1) ?? null,
    last_message: lastMessage,
    last_message_json: lastMessageJson,
    event_summary: captured.eventSummary ?? null,
  };
}
