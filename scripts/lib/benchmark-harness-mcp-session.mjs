import { spawn } from 'node:child_process';
import readline from 'node:readline';
import { closeChildProcess } from './child-process.mjs';
import { nowMs, roundMs } from './benchmark-harness-metrics.mjs';

export function parseToolPayload(response) {
  if (response.result?.isError) {
    const message = response.result.content?.[0]?.text ?? 'Unknown MCP tool error';
    throw new Error(message);
  }

  const text = response.result?.content?.[0]?.text;
  if (typeof text !== 'string') {
    throw new Error('Missing MCP text payload');
  }

  return JSON.parse(text);
}

function buildMcpSessionEnv({ extraEnv, homeOverride, skipGrammarDownload }) {
  return {
    ...process.env,
    ...extraEnv,
    HOME: homeOverride,
    SENTRUX_SKIP_GRAMMAR_DOWNLOAD: skipGrammarDownload,
  };
}

function attachMcpStdoutReader({ child, pending, stdoutLog, stderrLog }) {
  const stdoutReader = readline.createInterface({ input: child.stdout });
  stdoutReader.on('line', function onStdoutLine(line) {
    const trimmed = line.trim();
    if (!trimmed) {
      return;
    }
    if (!trimmed.startsWith('{')) {
      stdoutLog.push(trimmed);
      return;
    }

    let payload;
    try {
      payload = JSON.parse(trimmed);
    } catch {
      stderrLog.push(`Failed to parse MCP JSON: ${trimmed}`);
      return;
    }

    const handler = pending.get(payload.id);
    if (!handler) {
      stdoutLog.push(trimmed);
      return;
    }

    clearTimeout(handler.timer);
    pending.delete(payload.id);
    handler.resolve(payload);
  });
}

function attachMcpStderrReader({ child, stderrLog }) {
  const stderrReader = readline.createInterface({ input: child.stderr });
  stderrReader.on('line', function onStderrLine(line) {
    const trimmed = line.trim();
    if (trimmed) {
      stderrLog.push(trimmed);
    }
  });
}

function attachMcpExitHandler({ child, pending, markClosed }) {
  child.once('exit', function onExit(code, signal) {
    markClosed();
    for (const { reject, timer } of pending.values()) {
      clearTimeout(timer);
      reject(
        new Error(
          `MCP session exited before response (code=${code ?? 'null'}, signal=${signal ?? 'null'})`,
        ),
      );
    }
    pending.clear();
  });
}

function createMcpToolCaller({ child, pending, requestTimeoutMs, isClosed }) {
  let nextId = 1;

  return function callTool(name, argumentsObject) {
    if (isClosed()) {
      throw new Error('MCP session already closed');
    }

    const id = nextId++;
    const message = JSON.stringify({
      jsonrpc: '2.0',
      id,
      method: 'tools/call',
      params: {
        name,
        arguments: argumentsObject,
      },
    });

    return new Promise(function waitForToolResponse(resolve, reject) {
      const timer = setTimeout(function onTimeout() {
        pending.delete(id);
        reject(new Error(`Timed out waiting for MCP response for tool '${name}'`));
      }, requestTimeoutMs);

      pending.set(id, { resolve, reject, timer });
      child.stdin.write(`${message}\n`, function onWrite(error) {
        if (!error) {
          return;
        }
        clearTimeout(timer);
        pending.delete(id);
        reject(error);
      });
    });
  };
}

export function createMcpSession({
  binPath,
  repoRoot,
  homeOverride,
  skipGrammarDownload,
  requestTimeoutMs,
  extraEnv = {},
}) {
  const child = spawn(binPath, ['mcp'], {
    cwd: repoRoot,
    env: buildMcpSessionEnv({ extraEnv, homeOverride, skipGrammarDownload }),
    stdio: ['pipe', 'pipe', 'pipe'],
  });
  const pending = new Map();
  const stdoutLog = [];
  const stderrLog = [];
  let closed = false;

  attachMcpStdoutReader({ child, pending, stdoutLog, stderrLog });
  attachMcpStderrReader({ child, stderrLog });
  attachMcpExitHandler({
    child,
    pending,
    markClosed() {
      closed = true;
    },
  });
  const callTool = createMcpToolCaller({
    child,
    pending,
    requestTimeoutMs,
    isClosed() {
      return closed;
    },
  });

  async function close() {
    if (closed) {
      return;
    }

    await closeChildProcess(child);
  }

  return {
    callTool,
    close,
    stdoutLog,
    stderrLog,
  };
}

export async function runTool(session, name, argumentsObject) {
  const startedAt = nowMs();
  const response = await session.callTool(name, argumentsObject);
  const elapsedMs = roundMs(nowMs() - startedAt);

  return {
    elapsed_ms: elapsedMs,
    payload: parseToolPayload(response),
  };
}

export async function runBenchmarkTool(session, label, name, argumentsObject, summarize) {
  const result = await runTool(session, name, argumentsObject);

  return {
    label,
    tool: name,
    elapsed_ms: result.elapsed_ms,
    summary: summarize(result.payload),
  };
}
