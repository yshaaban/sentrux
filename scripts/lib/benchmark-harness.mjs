import { spawn } from 'node:child_process';
import { existsSync } from 'node:fs';
import { cp, readFile, rename, rm } from 'node:fs/promises';
import readline from 'node:readline';
import { closeChildProcess } from './child-process.mjs';
import { compareFileIdentity, compareRepoIdentity } from './repo-identity.mjs';

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

export function readPositiveInteger(value, fallback) {
  const parsed = Number.parseInt(String(value), 10);
  if (!Number.isInteger(parsed) || parsed <= 0) {
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
      const summaryKey = `${metric.classification}_count`;
      if (summaryKey in summary) {
        summary[summaryKey] += 1;
      }
      return summary;
    },
    {
      total: 0,
      fail_count: 0,
      warn_count: 0,
      info_count: 0,
      blocked_count: 0,
    },
  );
}

export function getBenchmarkMetric(benchmark, metricPath) {
  return metricPath.split('.').reduce((value, key) => value?.[key], benchmark);
}

export function setBenchmarkMetric(benchmark, metricPath, value) {
  const parts = metricPath.split('.');
  const lastPart = parts.pop();
  if (!lastPart) {
    return;
  }

  let current = benchmark;
  for (const part of parts) {
    if (!current || typeof current !== 'object' || !(part in current)) {
      return;
    }
    current = current[part];
  }

  if (!current || typeof current !== 'object') {
    return;
  }

  current[lastPart] = value;
}

function sortNumericValues(values) {
  return [...values].sort(function compareNumbers(left, right) {
    return left - right;
  });
}

function collectBenchmarkTimingMetricPaths(benchmark, currentPath = '', metricPaths = new Set()) {
  if (!benchmark || typeof benchmark !== 'object' || Array.isArray(benchmark)) {
    return metricPaths;
  }

  for (const [key, value] of Object.entries(benchmark)) {
    const metricPath = currentPath ? `${currentPath}.${key}` : key;
    if (typeof value === 'number' && (key === 'elapsed_ms' || key.endsWith('_total_ms'))) {
      metricPaths.add(metricPath);
      continue;
    }

    collectBenchmarkTimingMetricPaths(value, metricPath, metricPaths);
  }

  return metricPaths;
}

export function listBenchmarkTimingMetricPaths(benchmark) {
  return [...collectBenchmarkTimingMetricPaths(benchmark)].sort();
}

function computeMedianValue(sortedValues) {
  const middleIndex = Math.floor(sortedValues.length / 2);
  if (sortedValues.length % 2 === 1) {
    return roundMs(sortedValues[middleIndex]);
  }

  return roundMs((sortedValues[middleIndex - 1] + sortedValues[middleIndex]) / 2);
}

function computeMeanValue(values) {
  const sum = values.reduce(function add(total, value) {
    return total + value;
  }, 0);
  return sum / values.length;
}

function computeStandardDeviation(values, mean) {
  const variance =
    values.reduce(function addVariance(total, value) {
      return total + (value - mean) ** 2;
    }, 0) / values.length;

  return roundMs(Math.sqrt(variance));
}

export function buildMetricStatistics(values) {
  if (!Array.isArray(values) || values.length === 0) {
    return null;
  }

  const sortedValues = sortNumericValues(values);
  const meanValue = computeMeanValue(values);
  const meanMs = roundMs(meanValue);
  const medianMs = computeMedianValue(sortedValues);
  const minMs = roundMs(sortedValues[0]);
  const maxMs = roundMs(sortedValues.at(-1));

  return {
    sample_count: values.length,
    values_ms: values.map(roundMs),
    min_ms: minMs,
    max_ms: maxMs,
    median_ms: medianMs,
    mean_ms: meanMs,
    stddev_ms: computeStandardDeviation(values, meanValue),
    spread_ms: roundMs(maxMs - minMs),
  };
}

function listAggregateMetricPaths(samples, trackedMetrics) {
  const metricPaths = new Set(
    Array.isArray(trackedMetrics)
      ? trackedMetrics.map(function readTrackedMetric([metricPath]) {
          return metricPath;
        })
      : [],
  );

  for (const sample of samples) {
    for (const metricPath of listBenchmarkTimingMetricPaths(sample.benchmark)) {
      metricPaths.add(metricPath);
    }
  }

  return [...metricPaths].sort();
}

function buildBenchmarkDistance({ benchmark, aggregateBenchmark, trackedMetrics }) {
  return trackedMetrics.reduce(function sumDistance(totalDistance, [metricPath]) {
    const benchmarkValue = getBenchmarkMetric(benchmark, metricPath);
    const aggregateValue = getBenchmarkMetric(aggregateBenchmark, metricPath);
    if (!Number.isFinite(benchmarkValue) || !Number.isFinite(aggregateValue)) {
      return totalDistance;
    }

    return totalDistance + Math.abs(benchmarkValue - aggregateValue);
  }, 0);
}

function pickRepresentativeBenchmarkSample({ samples, aggregateBenchmark, trackedMetrics }) {
  let bestSampleIndex = 0;
  let bestDistance = Number.POSITIVE_INFINITY;

  for (let index = 0; index < samples.length; index += 1) {
    const distance = buildBenchmarkDistance({
      benchmark: samples[index].benchmark,
      aggregateBenchmark,
      trackedMetrics,
    });
    if (distance < bestDistance) {
      bestDistance = distance;
      bestSampleIndex = index;
    }
  }

  return bestSampleIndex;
}

function stringifyFreshnessMetadata(freshnessMetadata) {
  return JSON.stringify(freshnessMetadata);
}

function stripBenchmarkLogs(benchmark) {
  delete benchmark.stdout_log;
  delete benchmark.stderr_log;
  return benchmark;
}

function applyMetricStatisticsToBenchmark({ benchmark, statistics }) {
  for (const [metricPath, metricStatistics] of Object.entries(statistics)) {
    setBenchmarkMetric(benchmark, metricPath, metricStatistics.median_ms);
  }

  return benchmark;
}

export function buildAggregatedBenchmark({ samples, trackedMetrics }) {
  if (!Array.isArray(samples) || samples.length === 0) {
    return null;
  }

  const metricPaths = listAggregateMetricPaths(samples, trackedMetrics);
  const statistics = {};
  for (const metricPath of metricPaths) {
    const values = samples
      .map(function readMetric(sample) {
        return getBenchmarkMetric(sample.benchmark, metricPath);
      })
      .filter(Number.isFinite);
    if (values.length !== samples.length) {
      continue;
    }

    const metricStatistics = buildMetricStatistics(values);
    if (!metricStatistics) {
      continue;
    }

    statistics[metricPath] = metricStatistics;
  }

  const medianBenchmark = applyMetricStatisticsToBenchmark({
    benchmark: stripBenchmarkLogs(structuredClone(samples[0].benchmark)),
    statistics,
  });

  const representativeSampleIndex = pickRepresentativeBenchmarkSample({
    samples,
    aggregateBenchmark: medianBenchmark,
    trackedMetrics: metricPaths.map(function createTrackedMetric(metricPath) {
      return [metricPath];
    }),
  });

  const representativeBenchmarkSource =
    samples[representativeSampleIndex]?.benchmark ?? samples[0].benchmark;
  const aggregateBenchmark = applyMetricStatisticsToBenchmark({
    benchmark: stripBenchmarkLogs(structuredClone(representativeBenchmarkSource)),
    statistics,
  });

  return {
    aggregate_benchmark: aggregateBenchmark,
    metric_statistics: statistics,
    representative_sample_index: representativeSampleIndex,
    representative_sample_id: samples[representativeSampleIndex]?.sample_id ?? null,
    sample_count: samples.length,
  };
}

export async function runRepeatedBenchmarkSamples({ repeatCount, runSample }) {
  const samples = [];
  let referenceFreshnessMetadata = null;
  let referenceFreshnessKey = null;

  for (let sampleIndex = 0; sampleIndex < repeatCount; sampleIndex += 1) {
    const sample = await runSample(sampleIndex);
    const sampleFreshnessKey = stringifyFreshnessMetadata(sample.freshnessMetadata);
    if (referenceFreshnessKey === null) {
      referenceFreshnessKey = sampleFreshnessKey;
      referenceFreshnessMetadata = sample.freshnessMetadata;
    } else if (sampleFreshnessKey !== referenceFreshnessKey) {
      throw new Error(
        `Benchmark freshness metadata changed during repeated runs at ${sample.sample_id}`,
      );
    }

    samples.push({
      sample_id: sample.sample_id,
      generated_at: sample.generated_at,
      benchmark: sample.benchmark,
    });
  }

  return {
    samples,
    freshnessMetadata: referenceFreshnessMetadata,
  };
}

function collectIdentityMismatches({ field, currentValue, previousValue, compare }) {
  if (!currentValue && !previousValue) {
    return [];
  }

  return compare(previousValue, currentValue).map(function decorateMismatch(mismatch) {
    let mismatchKey = mismatch.key;
    if (typeof mismatchKey === 'string' && mismatchKey.startsWith(`${field}.`)) {
      mismatchKey = mismatchKey.slice(field.length + 1);
    }

    return {
      scope: field,
      key: mismatchKey,
      expected: mismatch.expected ?? null,
      actual: mismatch.actual ?? null,
    };
  });
}

export function buildBenchmarkComparability(currentResult, previousResult) {
  const mismatches = [
    ...collectIdentityMismatches({
      field: 'source_tree_identity',
      currentValue: currentResult?.source_tree_identity,
      previousValue: previousResult?.source_tree_identity,
      compare: compareRepoIdentity,
    }),
    ...collectIdentityMismatches({
      field: 'analyzed_tree_identity',
      currentValue: currentResult?.analyzed_tree_identity,
      previousValue: previousResult?.analyzed_tree_identity,
      compare: compareRepoIdentity,
    }),
    ...collectIdentityMismatches({
      field: 'rules_identity',
      currentValue: currentResult?.rules_identity,
      previousValue: previousResult?.rules_identity,
      compare: function compareRules(previousRules, currentRules) {
        return compareFileIdentity(previousRules, currentRules, 'rules_identity');
      },
    }),
    ...collectIdentityMismatches({
      field: 'binary_identity',
      currentValue: currentResult?.binary_identity,
      previousValue: previousResult?.binary_identity,
      compare: function compareBinary(previousBinary, currentBinary) {
        return compareFileIdentity(previousBinary, currentBinary, 'binary_identity');
      },
    }),
  ];

  return {
    comparable: mismatches.length === 0,
    mismatches,
  };
}

function buildComparisonMetric({
  currentResult,
  previousResult,
  benchmarkPolicy,
  metricPath,
  label,
  comparable,
}) {
  const previousValue = getBenchmarkMetric(previousResult.benchmark, metricPath);
  const currentValue = getBenchmarkMetric(currentResult.benchmark, metricPath);
  if (!Number.isFinite(previousValue) || !Number.isFinite(currentValue)) {
    return null;
  }

  const deltaMs = Number((currentValue - previousValue).toFixed(1));
  const deltaPercent = safePercent(deltaMs, previousValue);
  const classification = comparable
    ? classifyBenchmarkMetric(deltaMs, deltaPercent, benchmarkPolicy)
    : 'blocked';

  return {
    metric: label,
    path: metricPath,
    previous_ms: previousValue,
    current_ms: currentValue,
    delta_ms: deltaMs,
    delta_percent: deltaPercent,
    classification,
  };
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

  const comparability = buildBenchmarkComparability(currentResult, previousResult);
  const metrics = trackedMetrics
    .map(function createMetric([metricPath, label]) {
      return buildComparisonMetric({
        currentResult,
        previousResult,
        benchmarkPolicy,
        metricPath,
        label,
        comparable: comparability.comparable,
      });
    })
    .filter(Boolean);

  return {
    compared_to: compareToPath,
    previous_generated_at: previousResult.generated_at ?? null,
    comparable: comparability.comparable,
    comparability,
    aggregate_basis: {
      current: currentResult.benchmark_aggregate_basis ?? 'single_sample',
      previous: previousResult.benchmark_aggregate_basis ?? 'single_sample',
      current_sample_count: currentResult.benchmark_repeat_count ?? 1,
      previous_sample_count: previousResult.benchmark_repeat_count ?? 1,
    },
    policy: {
      fail: benchmarkPolicy.fail,
      warn: benchmarkPolicy.warn,
      mode: 'delta_ms_and_percent',
    },
    metrics,
    summary: summarizeComparisonMetrics(metrics),
    regressions: metrics.filter((metric) => metric.classification === 'fail'),
    warnings: metrics.filter((metric) => metric.classification === 'warn'),
    blocked_metrics: metrics.filter((metric) => metric.classification === 'blocked'),
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

  if (!comparison.comparable) {
    console.log('Benchmark comparison is informational only because the benchmark inputs changed.');
    for (const mismatch of comparison.comparability.mismatches) {
      const expectedValue =
        typeof mismatch.expected === 'string'
          ? `"${mismatch.expected}"`
          : JSON.stringify(mismatch.expected);
      const actualValue =
        typeof mismatch.actual === 'string' ? `"${mismatch.actual}"` : JSON.stringify(mismatch.actual);
      console.log(`- ${mismatch.scope}.${mismatch.key}: expected ${expectedValue}, got ${actualValue}`);
    }
    console.log(
      `Recorded ${comparison.blocked_metrics.length} benchmark delta(s) without gating because the compared artifacts are not comparable.`,
    );
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
