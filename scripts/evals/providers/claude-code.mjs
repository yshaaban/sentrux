#!/usr/bin/env node

import { spawn, spawnSync } from 'node:child_process';
import { existsSync } from 'node:fs';

const defaultClaudeBin = process.env.CLAUDE_BIN ?? 'claude';
let cachedVersion = undefined;

function nowMs() {
  return Number(process.hrtime.bigint()) / 1_000_000;
}

function parseJsonMaybe(text) {
  if (typeof text !== 'string') {
    return null;
  }

  const trimmed = text.trim();
  if (!trimmed) {
    return null;
  }

  try {
    return JSON.parse(trimmed);
  } catch {
    return null;
  }
}

function spawnCaptured(command, args, options) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, {
      cwd: options.cwd,
      env: options.env,
      stdio: ['ignore', 'pipe', 'pipe'],
      shell: false,
    });

    let stdout = '';
    let stderr = '';
    let timedOut = false;
    const timeout =
      options.timeoutMs && options.timeoutMs > 0
        ? setTimeout(() => {
            timedOut = true;
            child.kill('SIGKILL');
          }, options.timeoutMs)
        : null;

    child.stdout.on('data', (chunk) => {
      stdout += chunk.toString('utf8');
    });

    child.stderr.on('data', (chunk) => {
      stderr += chunk.toString('utf8');
    });

    child.on('error', (error) => {
      if (timeout) {
        clearTimeout(timeout);
      }
      reject(error);
    });

    child.on('close', (exitCode, signal) => {
      if (timeout) {
        clearTimeout(timeout);
      }

      resolve({
        exitCode,
        signal,
        stdout,
        stderr,
        timedOut,
      });
    });
  });
}

function resolveClaudeVersion(claudeBin) {
  if (cachedVersion !== undefined) {
    return cachedVersion;
  }

  const result = spawnSync(claudeBin, ['--version'], {
    encoding: 'utf8',
    shell: false,
    stdio: ['ignore', 'pipe', 'pipe'],
  });

  if (result.status === 0) {
    cachedVersion = result.stdout.trim() || null;
    return cachedVersion;
  }

  cachedVersion = null;
  return cachedVersion;
}

export async function runClaudeCode(options = {}) {
  const {
    cwd,
    prompt,
    model,
    jsonSchema,
    appendSystemPrompt,
    permissionMode = 'bypassPermissions',
    tools = 'default',
    addDirs = [],
    timeoutMs = 30 * 60 * 1000,
    claudeBin = defaultClaudeBin,
    env = process.env,
  } = options;

  if (typeof cwd !== 'string' || !cwd) {
    throw new Error('runClaudeCode requires a cwd');
  }

  if (!existsSync(cwd)) {
    throw new Error(`Claude Code cwd does not exist: ${cwd}`);
  }

  if (typeof prompt !== 'string' || !prompt.trim()) {
    throw new Error('runClaudeCode requires a non-empty prompt');
  }

  const args = [
    '-p',
    '--output-format',
    'json',
    '--input-format',
    'text',
    '--no-session-persistence',
    '--permission-mode',
    permissionMode,
  ];

  if (tools) {
    args.push('--tools', tools);
  }

  if (appendSystemPrompt) {
    args.push('--append-system-prompt', appendSystemPrompt);
  }

  if (jsonSchema) {
    args.push(
      '--json-schema',
      typeof jsonSchema === 'string' ? jsonSchema : JSON.stringify(jsonSchema),
    );
  }

  if (model) {
    args.push('--model', model);
  }

  for (const dir of addDirs) {
    if (typeof dir === 'string' && dir) {
      args.push('--add-dir', dir);
    }
  }

  args.push(prompt);

  const startedAt = new Date().toISOString();
  const startedMs = nowMs();
  const captured = await spawnCaptured(claudeBin, args, {
    cwd,
    env,
    timeoutMs,
  });
  const durationMs = Number((nowMs() - startedMs).toFixed(1));

  return {
    provider: 'claude-code',
    provider_version: resolveClaudeVersion(claudeBin),
    command: {
      executable: claudeBin,
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
    stdout_json: parseJsonMaybe(captured.stdout),
  };
}
