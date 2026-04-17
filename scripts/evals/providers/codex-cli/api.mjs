import { existsSync } from 'node:fs';
import { rm } from 'node:fs/promises';

import { defaultCodexBin, nowMs, resolveCodexVersion } from './shared.mjs';
import { buildCodexExecArgs, createInvocationFiles, readLastMessage } from './invocation.mjs';
import { startCaptured } from './process-capture.mjs';
import { buildCodexResult } from './result.mjs';

function validateOptions(apiName, cwd, prompt) {
  if (typeof cwd !== 'string' || !cwd) {
    throw new Error(`${apiName} requires a cwd`);
  }
  if (!existsSync(cwd)) {
    throw new Error(`Codex cwd does not exist: ${cwd}`);
  }
  if (typeof prompt !== 'string' || !prompt.trim()) {
    throw new Error(`${apiName} requires a non-empty prompt`);
  }
}

export async function startCodexExec(options = {}) {
  const {
    cwd,
    prompt,
    model = null,
    jsonSchema = null,
    timeoutMs = 30 * 60 * 1000,
    codexBin = defaultCodexBin,
    env = process.env,
    addDirs = [],
    sandbox = null,
    config = [],
  } = options;

  validateOptions('startCodexExec', cwd, prompt);

  const invocationFiles = await createInvocationFiles(jsonSchema);
  const args = buildCodexExecArgs(
    {
      cwd,
      prompt,
      model,
      addDirs,
      sandbox,
      config,
    },
    invocationFiles,
  );
  const startedAt = new Date().toISOString();
  const startedMs = nowMs();
  const captured = startCaptured(codexBin, args, {
    cwd,
    env,
    timeoutMs,
  });

  async function wait() {
    try {
      const result = await captured.wait();
      const durationMs = Number((nowMs() - startedMs).toFixed(1));
      const lastMessage = await readLastMessage(invocationFiles.lastMessagePath);

      return buildCodexResult({
        codexBin,
        args,
        cwd,
        startedAt,
        durationMs,
        captured: result,
        lastMessage,
      });
    } finally {
      await rm(invocationFiles.tempRoot, { recursive: true, force: true });
    }
  }

  return {
    provider: 'codex-cli',
    provider_version: resolveCodexVersion(codexBin),
    command: {
      executable: codexBin,
      args,
    },
    cwd,
    started_at: startedAt,
    pid: captured.child.pid ?? null,
    child: captured.child,
    get finished() {
      return captured.finished;
    },
    get stdoutLength() {
      return captured.stdoutLength;
    },
    get stderrLength() {
      return captured.stderrLength;
    },
    get lastOutputAtMs() {
      return captured.lastOutputAtMs;
    },
    get eventSummary() {
      return captured.eventSummary;
    },
    kill(signal) {
      captured.kill(signal);
    },
    wait,
  };
}

export async function runCodexExec(options = {}) {
  const {
    cwd,
    prompt,
    model = null,
    jsonSchema = null,
    timeoutMs = 30 * 60 * 1000,
    codexBin = defaultCodexBin,
    env = process.env,
    addDirs = [],
    sandbox = null,
    config = [],
  } = options;

  validateOptions('runCodexExec', cwd, prompt);

  const running = await startCodexExec({
    cwd,
    prompt,
    model,
    jsonSchema,
    timeoutMs,
    codexBin,
    env,
    addDirs,
    sandbox,
    config,
  });

  return running.wait();
}
