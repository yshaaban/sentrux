import { existsSync } from 'node:fs';
import { readFile } from 'node:fs/promises';
import { compareFileIdentity, compareRepoIdentity } from './repo-identity.mjs';
import {
  classifyBenchmarkMetric,
  getBenchmarkMetric,
  safePercent,
  summarizeComparisonMetrics,
} from './benchmark-harness-metrics.mjs';

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
    regressions: metrics.filter(function isRegression(metric) {
      return metric.classification === 'fail';
    }),
    warnings: metrics.filter(function isWarning(metric) {
      return metric.classification === 'warn';
    }),
    blocked_metrics: metrics.filter(function isBlocked(metric) {
      return metric.classification === 'blocked';
    }),
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
