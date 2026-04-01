import { spawn } from 'node:child_process';
import { existsSync } from 'node:fs';
import { cp, readFile, rename, rm } from 'node:fs/promises';
import readline from 'node:readline';

export function roundMs(value) {
  return Number(value.toFixed(1));
}

export function nowMs() {
  return Number(process.hrtime.bigint()) / 1_000_000;
}

export function readNonNegativeNumber(value, fallback) {
  const parsed = Number(value);
  if (!Number.isFinite(parsed) || parsed < 0) {
    return fallback;
  }

  return parsed;
}

export function buildBenchmarkPolicy(env = process.env) {
  const fail = {
    max_regression_ms: readNonNegativeNumber(env.MAX_REGRESSION_MS, 250),
    max_regression_percent: readNonNegativeNumber(env.MAX_REGRESSION_PERCENT, 20),
  };
  const warn = {
    max_regression_ms: Math.min(
      readNonNegativeNumber(env.WARN_REGRESSION_MS, 150),
      fail.max_regression_ms,
    ),
    max_regression_percent: Math.min(
      readNonNegativeNumber(env.WARN_REGRESSION_PERCENT, 10),
      fail.max_regression_percent,
    ),
  };

  return { fail, warn };
}

export function safePercent(delta, baseline) {
  if (!Number.isFinite(delta) || !Number.isFinite(baseline) || baseline === 0) {
    return null;
  }

  return Number(((delta / baseline) * 100).toFixed(1));
}

export function classifyBenchmarkMetric(deltaMs, deltaPercent, benchmarkPolicy) {
  const exceedsFail =
    deltaMs > benchmarkPolicy.fail.max_regression_ms &&
    deltaPercent !== null &&
    deltaPercent > benchmarkPolicy.fail.max_regression_percent;
  if (exceedsFail) {
    return 'fail';
  }

  const exceedsWarn =
    deltaMs > benchmarkPolicy.warn.max_regression_ms &&
    deltaPercent !== null &&
    deltaPercent > benchmarkPolicy.warn.max_regression_percent;
  if (exceedsWarn) {
    return 'warn';
  }

  return 'info';
}

export function summarizeComparisonMetrics(metrics) {
  return metrics.reduce(
    (summary, metric) => {
      summary.total += 1;
      summary[`${metric.classification}_count`] += 1;
      return summary;
    },
    {
      total: 0,
      fail_count: 0,
      warn_count: 0,
      info_count: 0,
    },
  );
}

export function getBenchmarkMetric(benchmark, metricPath) {
  return metricPath.split('.').reduce((value, key) => value?.[key], benchmark);
}

export function buildBenchmarkComparison({
  currentResult,
  previousResult,
  compareToPath,
  benchmarkPolicy,
  trackedMetrics,
}) {
  if (
    !previousResult?.benchmark ||
    previousResult.benchmark_format_version !== currentResult.benchmark_format_version
  ) {
    return null;
  }

  const metrics = trackedMetrics
    .map(([metricPath, label]) => {
      const previousValue = getBenchmarkMetric(previousResult.benchmark, metricPath);
      const currentValue = getBenchmarkMetric(currentResult.benchmark, metricPath);
      if (!Number.isFinite(previousValue) || !Number.isFinite(currentValue)) {
        return null;
      }

      const deltaMs = Number((currentValue - previousValue).toFixed(1));
      const deltaPercent = safePercent(deltaMs, previousValue);
      const classification = classifyBenchmarkMetric(deltaMs, deltaPercent, benchmarkPolicy);

      return {
        metric: label,
        path: metricPath,
        previous_ms: previousValue,
        current_ms: currentValue,
        delta_ms: deltaMs,
        delta_percent: deltaPercent,
        classification,
      };
    })
    .filter(Boolean);

  return {
    compared_to: compareToPath,
    previous_generated_at: previousResult.generated_at ?? null,
    policy: {
      fail: benchmarkPolicy.fail,
      warn: benchmarkPolicy.warn,
      mode: 'delta_ms_and_percent',
    },
    metrics,
    summary: summarizeComparisonMetrics(metrics),
    regressions: metrics.filter((metric) => metric.classification === 'fail'),
    warnings: metrics.filter((metric) => metric.classification === 'warn'),
  };
}

export function printBenchmarkPolicy(policy) {
  console.log(
    `Benchmark policy: fail at >${policy.fail.max_regression_ms}ms and >${policy.fail.max_regression_percent}%; warn at >${policy.warn.max_regression_ms}ms and >${policy.warn.max_regression_percent}%`,
  );
}

export function printComparisonMetrics(countLabel, severity, metrics) {
  if (!metrics.length) {
    return;
  }

  console.log(`Detected ${metrics.length} ${countLabel}.`);
  for (const metric of metrics) {
    console.log(
      `- [${severity}] ${metric.metric}: ${metric.previous_ms}ms -> ${metric.current_ms}ms (${metric.delta_ms}ms, ${metric.delta_percent}%)`,
    );
  }
}

export function printBenchmarkComparison(comparison) {
  if (!comparison) {
    console.log('No prior benchmark artifact available for comparison.');
    return;
  }

  printBenchmarkPolicy(comparison.policy);
  printComparisonMetrics('benchmark fail regression(s)', 'fail', comparison.regressions);
  printComparisonMetrics('benchmark warning regression(s)', 'warn', comparison.warnings);

  if (comparison.regressions.length === 0 && comparison.warnings.length === 0) {
    console.log('No benchmark regressions detected.');
  }
}

export async function loadPreviousBenchmark(comparePath) {
  if (!existsSync(comparePath)) {
    return null;
  }

  const raw = await readFile(comparePath, 'utf8');
  return JSON.parse(raw);
}

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

export function createMcpSession({
  binPath,
  repoRoot,
  homeOverride,
  skipGrammarDownload,
  requestTimeoutMs,
}) {
  const child = spawn(binPath, ['--mcp'], {
    cwd: repoRoot,
    env: {
      ...process.env,
      HOME: homeOverride,
      SENTRUX_SKIP_GRAMMAR_DOWNLOAD: skipGrammarDownload,
    },
    stdio: ['pipe', 'pipe', 'pipe'],
  });
  const pending = new Map();
  const stdoutLog = [];
  const stderrLog = [];
  let nextId = 1;
  let closed = false;

  const stdoutReader = readline.createInterface({ input: child.stdout });
  stdoutReader.on('line', (line) => {
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
    } catch (error) {
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

  const stderrReader = readline.createInterface({ input: child.stderr });
  stderrReader.on('line', (line) => {
    const trimmed = line.trim();
    if (trimmed) {
      stderrLog.push(trimmed);
    }
  });

  child.once('exit', (code, signal) => {
    closed = true;
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

  function callTool(name, argumentsObject) {
    if (closed) {
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

    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        pending.delete(id);
        reject(new Error(`Timed out waiting for MCP response for tool '${name}'`));
      }, requestTimeoutMs);

      pending.set(id, { resolve, reject, timer });
      child.stdin.write(`${message}\n`, (error) => {
        if (!error) {
          return;
        }
        clearTimeout(timer);
        pending.delete(id);
        reject(error);
      });
    });
  }

  async function close() {
    if (closed) {
      return;
    }

    child.stdin.end();
    await new Promise((resolve) => {
      child.once('exit', () => resolve());
    });
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

export async function runCommand(command, args, options = {}) {
  const {
    cwd = process.cwd(),
    env = {},
    homeOverride = null,
    input = null,
    skipGrammarDownload = null,
  } = options;
  const startedAt = nowMs();
  const child = spawn(command, args, {
    cwd,
    env: {
      ...process.env,
      ...env,
      ...(homeOverride ? { HOME: homeOverride } : {}),
      ...(skipGrammarDownload ? { SENTRUX_SKIP_GRAMMAR_DOWNLOAD: skipGrammarDownload } : {}),
    },
    stdio: ['pipe', 'pipe', 'pipe'],
  });

  let stdout = '';
  let stderr = '';
  child.stdout.setEncoding('utf8');
  child.stderr.setEncoding('utf8');
  child.stdout.on('data', (chunk) => {
    stdout += chunk;
  });
  child.stderr.on('data', (chunk) => {
    stderr += chunk;
  });

  const exit = new Promise((resolve, reject) => {
    child.once('error', reject);
    child.once('close', (exitCode, signal) => {
      resolve({
        exit_code: exitCode ?? null,
        signal: signal ?? null,
      });
    });
  });

  if (input !== null) {
    child.stdin.end(input);
  } else {
    child.stdin.end();
  }

  const result = await exit;
  return {
    elapsed_ms: roundMs(nowMs() - startedAt),
    exit_code: result.exit_code,
    signal: result.signal,
    stdout,
    stderr,
  };
}

export async function runBenchmarkCommand(command, label, args, summarize, options = {}) {
  const result = await runCommand(command, args, options);

  return {
    label,
    command,
    args,
    elapsed_ms: result.elapsed_ms,
    summary: summarize(result),
    exit_code: result.exit_code,
    signal: result.signal,
  };
}

export async function backupFileIfExists(targetPath, backupPath) {
  if (!existsSync(targetPath)) {
    return false;
  }

  await cp(targetPath, backupPath);
  return true;
}

export async function restoreManagedFile(targetPath, backupPath, existedBefore) {
  if (existedBefore) {
    await rename(backupPath, targetPath);
    return;
  }

  if (existsSync(targetPath)) {
    await rm(targetPath, { force: true });
  }
}
