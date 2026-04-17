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
    function summarize(summary, metric) {
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
  return metricPath.split('.').reduce(function descend(value, key) {
    return value?.[key];
  }, benchmark);
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
