const FALLBACK_SEVERITY_SCORES = {
  high: 7000,
  medium: 5000,
  low: 3000,
  unknown: 1000,
};

function fallbackScore(severity) {
  switch (severity) {
    case 'high':
      return FALLBACK_SEVERITY_SCORES.high;
    case 'medium':
      return FALLBACK_SEVERITY_SCORES.medium;
    case 'low':
      return FALLBACK_SEVERITY_SCORES.low;
    default:
      return FALLBACK_SEVERITY_SCORES.unknown;
  }
}

function normalizeMetricKey(key) {
  return String(key ?? '')
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '_')
    .replace(/^_+|_+$/g, '');
}

function normalizeMetrics(metrics) {
  if (Array.isArray(metrics)) {
    return Object.fromEntries(
      metrics
        .filter((metric) => metric && metric.label)
        .map((metric) => [normalizeMetricKey(metric.label), metric.value]),
    );
  }

  if (metrics && typeof metrics === 'object') {
    return Object.fromEntries(
      Object.entries(metrics).map(([key, value]) => [normalizeMetricKey(key), value]),
    );
  }

  return {};
}

function metricValue(candidate, key) {
  const normalizedKey = normalizeMetricKey(key);
  const metric = candidate.metrics?.[normalizedKey];
  if (typeof metric === 'number') {
    return metric;
  }

  const directValue = candidate[normalizedKey];
  if (typeof directValue === 'number') {
    return directValue;
  }

  return 0;
}

function hasRoleTag(candidate, roleTag) {
  return (candidate.role_tags ?? []).includes(roleTag);
}

function bestCutCandidate(candidate) {
  return (candidate.cut_candidates ?? [])[0] ?? null;
}

function cappedBonus(value, unit, maxBonus) {
  return Math.min(value * unit, maxBonus);
}

function containsReason(candidate, reason) {
  return (candidate.leverage_reasons ?? []).includes(reason);
}

function defaultLeverageClass(presentationClass) {
  switch (presentationClass) {
    case 'guarded_facade':
      return 'boundary_discipline';
    case 'hardening_note':
      return 'hardening_note';
    case 'tooling_debt':
      return 'tooling_debt';
    case 'experimental':
      return 'experimental';
    default:
      return 'secondary_cleanup';
  }
}

function compactSelectedCandidate(candidate) {
  const current = candidate ?? {};
  return {
    kind: current.kind ?? null,
    trust_tier: current.trust_tier ?? null,
    presentation_class: current.presentation_class ?? null,
    leverage_class: current.leverage_class ?? null,
    leverage_reasons: current.leverage_reasons ?? [],
    ranking_reasons: current.ranking_reasons ?? [],
    scope: current.scope ?? null,
    severity: current.severity ?? null,
    score_band: current.score_band ?? null,
    score_0_10000: current.score_0_10000 ?? null,
    within_bucket_strength_0_10000: current.within_bucket_strength_0_10000 ?? null,
    summary: current.summary ?? null,
    impact: current.impact ?? null,
    candidate_split_axes: current.candidate_split_axes ?? [],
    related_surfaces: current.related_surfaces ?? [],
  };
}

function signalMatchesScope(signal, scope) {
  if (signal.scope === scope) {
    return true;
  }

  return (signal.files ?? []).includes(scope);
}

function normalizeCandidate(candidate, sourceType, matchingSignal = null, matchingClusters = []) {
  const presentationClass = candidate.presentation_class ?? 'structural_debt';
  const metrics = normalizeMetrics(candidate.metrics);
  return {
    ...candidate,
    metrics,
    role_tags: candidate.role_tags ?? [],
    cut_candidates: candidate.cut_candidates ?? [],
    source_type: sourceType,
    presentation_class: presentationClass,
    leverage_class: candidate.leverage_class ?? defaultLeverageClass(presentationClass),
    leverage_reasons: candidate.leverage_reasons ?? [],
    score_0_10000:
      candidate.score_0_10000 ?? matchingSignal?.score_0_10000 ?? fallbackScore(candidate.severity),
    signal_families: candidate.signal_families ?? matchingSignal?.signal_families ?? [],
    cluster_overlap_count: matchingClusters.length,
    cluster_signal_count: Math.max(
      0,
      ...matchingClusters.map((cluster) => cluster.metrics?.signal_count ?? 0),
    ),
    hotspot_overlap: hasHotspotOverlap(matchingSignal, matchingClusters),
  };
}

function hasHotspotOverlap(matchingSignal, matchingClusters) {
  if (matchingSignal && ['hotspot', 'unstable_hotspot'].includes(matchingSignal.kind)) {
    return true;
  }

  return matchingClusters.some((cluster) =>
    (cluster.signal_kinds ?? []).some((kind) => ['hotspot', 'unstable_hotspot'].includes(kind)),
  );
}

export {
  bestCutCandidate,
  cappedBonus,
  compactSelectedCandidate,
  containsReason,
  defaultLeverageClass,
  hasRoleTag,
  metricValue,
  normalizeCandidate,
  normalizeMetricKey,
  normalizeMetrics,
  signalMatchesScope,
};
