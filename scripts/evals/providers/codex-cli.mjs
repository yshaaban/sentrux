#!/usr/bin/env node

import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { spawn, spawnSync } from 'node:child_process';

const defaultCodexBin = process.env.CODEX_BIN ?? 'codex';
let cachedVersion = undefined;

function nowMs() {
  return Number(process.hrtime.bigint()) / 1_000_000;
}

function parseJsonLine(value) {
  if (typeof value !== 'string') {
    return null;
  }

  const trimmed = value.trim();
  if (!trimmed) {
    return null;
  }

  try {
    return JSON.parse(trimmed);
  } catch {
    return null;
  }
}

function parseJsonLines(value) {
  if (typeof value !== 'string') {
    return [];
  }

  return value
    .split(/\r?\n/)
    .map(parseJsonLine)
    .filter(Boolean);
}

function summarizeCommandItem(item) {
  if (!item || item.type !== 'command_execution') {
    return null;
  }

  return {
    command: typeof item.command === 'string' ? item.command : null,
    exit_code: Number.isInteger(item.exit_code) ? item.exit_code : null,
    status: typeof item.status === 'string' ? item.status : null,
  };
}

function createStdoutEventTracker() {
  let remainder = '';
  const summary = {
    event_count: 0,
    parsed_event_count: 0,
    unparsed_line_count: 0,
    turn_count: 0,
    command_started_count: 0,
    command_completed_count: 0,
    completed_item_count: 0,
    agent_message_count: 0,
    last_event_type: null,
    last_event_elapsed_ms: null,
    last_completed_item_type: null,
    last_completed_item_elapsed_ms: null,
    last_started_command: null,
    last_started_command_elapsed_ms: null,
    in_progress_command: null,
    in_progress_command_elapsed_ms: null,
    last_completed_command: null,
    last_completed_command_elapsed_ms: null,
    last_completed_command_exit_code: null,
    last_completed_command_status: null,
    last_agent_message: null,
    last_agent_message_elapsed_ms: null,
  };
  const startedMs = nowMs();

  function elapsedMs() {
    return Number((nowMs() - startedMs).toFixed(1));
  }

  function updateParsedEvent(parsed, eventElapsedMs) {
    summary.parsed_event_count += 1;
    summary.last_event_type = typeof parsed.type === 'string' ? parsed.type : null;
    summary.last_event_elapsed_ms = eventElapsedMs;
  }

  function handleStartedCommand(item, eventElapsedMs) {
    const command = summarizeCommandItem(item);

    summary.command_started_count += 1;
    summary.last_started_command = command;
    summary.last_started_command_elapsed_ms = eventElapsedMs;
    summary.in_progress_command = command;
    summary.in_progress_command_elapsed_ms = eventElapsedMs;
  }

  function handleCompletedCommand(item, eventElapsedMs) {
    const command = summarizeCommandItem(item);

    summary.command_completed_count += 1;
    summary.last_completed_command = command;
    summary.last_completed_command_elapsed_ms = eventElapsedMs;
    summary.last_completed_command_exit_code = command?.exit_code ?? null;
    summary.last_completed_command_status = command?.status ?? null;
    summary.in_progress_command = null;
    summary.in_progress_command_elapsed_ms = null;
  }

  function handleAgentMessage(item, eventElapsedMs) {
    summary.agent_message_count += 1;
    summary.last_agent_message = typeof item.text === 'string' ? item.text : null;
    summary.last_agent_message_elapsed_ms = eventElapsedMs;
  }

  function consumeLine(line) {
    if (!line.trim()) {
      return;
    }

    summary.event_count += 1;
    const parsed = parseJsonLine(line);
    if (!parsed) {
      summary.unparsed_line_count += 1;
      return;
    }

    const eventElapsedMs = elapsedMs();
    updateParsedEvent(parsed, eventElapsedMs);

    if (parsed.type === 'turn.started') {
      summary.turn_count += 1;
      return;
    }

    const item = parsed.item ?? null;
    const itemType = item?.type;
    if (typeof itemType !== 'string') {
      return;
    }

    switch (parsed.type) {
      case 'item.started':
        if (itemType === 'command_execution') {
          handleStartedCommand(item, eventElapsedMs);
        }
        return;
      case 'item.completed':
        summary.completed_item_count += 1;
        summary.last_completed_item_type = itemType;
        summary.last_completed_item_elapsed_ms = eventElapsedMs;

        if (itemType === 'command_execution') {
          handleCompletedCommand(item, eventElapsedMs);
          return;
        }
        if (itemType === 'agent_message') {
          handleAgentMessage(item, eventElapsedMs);
        }
        return;
      default:
        return;
    }
  }

  function consume(text) {
    remainder += text;
    const lines = remainder.split(/\r?\n/);
    remainder = lines.pop() ?? '';
    for (const line of lines) {
      consumeLine(line);
    }
  }

  function finish() {
    if (remainder) {
      consumeLine(remainder);
      remainder = '';
    }
  }

  function snapshot() {
    return JSON.parse(JSON.stringify(summary));
  }

  return {
    consume,
    finish,
    snapshot,
  };
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
      const text = chunk.toString('utf8');
      stdout += text;
    });
    child.stderr.on('data', (chunk) => {
      const text = chunk.toString('utf8');
      stderr += text;
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

function startCaptured(command, args, options) {
  const child = spawn(command, args, {
    cwd: options.cwd,
    env: options.env,
    stdio: ['ignore', 'pipe', 'pipe'],
    shell: false,
  });

  let stdout = '';
  let stderr = '';
  let stdoutLength = 0;
  let stderrLength = 0;
  let timedOut = false;
  let finished = false;
  let lastOutputAtMs = nowMs();
  const eventTracker = createStdoutEventTracker();
  const timeout =
    options.timeoutMs && options.timeoutMs > 0
      ? setTimeout(() => {
          timedOut = true;
          child.kill('SIGKILL');
        }, options.timeoutMs)
      : null;

  const resultPromise = new Promise((resolve, reject) => {
    child.stdout.on('data', (chunk) => {
      const text = chunk.toString('utf8');
      stdout += text;
      stdoutLength += text.length;
      lastOutputAtMs = nowMs();
      eventTracker.consume(text);
    });
    child.stderr.on('data', (chunk) => {
      const text = chunk.toString('utf8');
      stderr += text;
      stderrLength += text.length;
      lastOutputAtMs = nowMs();
    });
    child.on('error', (error) => {
      finished = true;
      if (timeout) {
        clearTimeout(timeout);
      }
      reject(error);
    });
    child.on('close', (exitCode, signal) => {
      finished = true;
      if (timeout) {
        clearTimeout(timeout);
      }
      eventTracker.finish();
      resolve({
        exitCode,
        signal,
        stdout,
        stderr,
        timedOut,
        eventSummary: eventTracker.snapshot(),
      });
    });
  });

  return {
    child,
    get finished() {
      return finished;
    },
    get stdoutLength() {
      return stdoutLength;
    },
    get stderrLength() {
      return stderrLength;
    },
    get lastOutputAtMs() {
      return lastOutputAtMs;
    },
    get eventSummary() {
      return eventTracker.snapshot();
    },
    kill(signal = 'SIGKILL') {
      if (!finished) {
        child.kill(signal);
      }
    },
    wait() {
      return resultPromise;
    },
  };
}

function resolveCodexVersion(codexBin) {
  if (cachedVersion !== undefined) {
    return cachedVersion;
  }

  const result = spawnSync(codexBin, ['--version'], {
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

async function createInvocationFiles(jsonSchema) {
  const tempRoot = await mkdtemp(path.join(tmpdir(), 'sentrux-codex-provider-'));
  const lastMessagePath = path.join(tempRoot, 'last-message.txt');
  let schemaPath = null;

  if (jsonSchema) {
    schemaPath = path.join(tempRoot, 'output-schema.json');
    await writeFile(
      schemaPath,
      `${typeof jsonSchema === 'string' ? jsonSchema : JSON.stringify(jsonSchema, null, 2)}\n`,
      'utf8',
    );
  }

  return {
    tempRoot,
    lastMessagePath,
    schemaPath,
  };
}

function buildCodexExecArgs(options, invocationFiles) {
  const args = [
    'exec',
    '--json',
    '--skip-git-repo-check',
    '--dangerously-bypass-approvals-and-sandbox',
    '--cd',
    options.cwd,
    '--output-last-message',
    invocationFiles.lastMessagePath,
  ];

  if (options.model) {
    args.push('--model', options.model);
  }
  if (options.sandbox) {
    args.push('--sandbox', options.sandbox);
  }
  for (const dir of options.addDirs ?? []) {
    if (typeof dir === 'string' && dir) {
      args.push('--add-dir', dir);
    }
  }
  if (invocationFiles.schemaPath) {
    args.push('--output-schema', invocationFiles.schemaPath);
  }
  for (const [key, value] of options.config ?? []) {
    args.push('--config', `${key}=${value}`);
  }

  args.push(options.prompt);
  return args;
}

async function readLastMessage(lastMessagePath) {
  if (!existsSync(lastMessagePath)) {
    return null;
  }

  return readFile(lastMessagePath, 'utf8');
}

function buildCodexResult({
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

  if (typeof cwd !== 'string' || !cwd) {
    throw new Error('startCodexExec requires a cwd');
  }
  if (!existsSync(cwd)) {
    throw new Error(`Codex cwd does not exist: ${cwd}`);
  }
  if (typeof prompt !== 'string' || !prompt.trim()) {
    throw new Error('startCodexExec requires a non-empty prompt');
  }

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

  if (typeof cwd !== 'string' || !cwd) {
    throw new Error('runCodexExec requires a cwd');
  }
  if (!existsSync(cwd)) {
    throw new Error(`Codex cwd does not exist: ${cwd}`);
  }
  if (typeof prompt !== 'string' || !prompt.trim()) {
    throw new Error('runCodexExec requires a non-empty prompt');
  }

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
