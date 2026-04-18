import { asArray, safeRatio } from './signal-summary-utils.mjs';
import { normalizeExperimentArm } from './experiment-arms.mjs';

const MISSED_EXPECTED_SIGNAL_BUCKETS = new Set([
  'missed_expected_signal',
  'clean_but_missed_expected_signal',
]);

const EXPECTED_SIGNAL_NOT_TOP_BUCKETS = new Set([
  'expected_signal_present_not_top',
  'clean_but_misranked',
]);

const TOP_ACTION_FAILURE_BUCKETS = [
  'missed_expected_signal',
  'clean_but_missed_expected_signal',
  'expected_signal_present_not_top',
  'clean_but_misranked',
  'regressed',
  'thrashing',
  'stalled',
  'provider_failed',
];

function uniqueSortedStrings(values) {
  return [...new Set(values.filter(Boolean).map(String))].sort((left, right) =>
    left.localeCompare(right),
  );
}

function focusAreasForExpectedSignals(expectedSignalKinds, tags) {
  const focusAreas = new Set();

  for (const signalKind of expectedSignalKinds) {
    if (signalKind.includes('clone')) {
      focusAreas.add('clone_followthrough');
      continue;
    }
    if (signalKind.includes('propagation') || signalKind.includes('exhaustiveness')) {
      focusAreas.add('propagation');
      continue;
    }
    if (
      signalKind.includes('boundary') ||
      signalKind.includes('raw_') ||
      signalKind.includes('writer')
    ) {
      focusAreas.add('boundary');
      continue;
    }
    focusAreas.add('structural');
  }

  for (const tag of tags) {
    if (tag === 'clone') {
      focusAreas.add('clone_followthrough');
    }
    if (tag === 'propagation') {
      focusAreas.add('propagation');
    }
    if (tag === 'governance' || tag === 'session') {
      focusAreas.add('session_governance');
    }
  }

  if (focusAreas.size === 0) {
    focusAreas.add('general');
  }

  return [...focusAreas].sort((left, right) => left.localeCompare(right));
}

function sessionIdForEntry(entry) {
  return entry.task_id ?? entry.replay_id ?? entry.commit ?? entry.task_label ?? 'session';
}

function sessionLabelForEntry(entry) {
  return entry.task_label ?? entry.commit ?? entry.replay_id ?? entry.task_id ?? 'session';
}

function normalizeInitialActionKinds(outcome) {
  return uniqueSortedStrings(asArray(outcome?.initial_action_kinds));
}

function buildEvidenceFlags(expectedSignalKinds, initialActionKinds, initialTopActionKind, outcome) {
  const expectedSignalSet = new Set(expectedSignalKinds);
  const initialActionSet = new Set(initialActionKinds);
  const expectedSignalsPresent = expectedSignalKinds.filter((kind) => initialActionSet.has(kind));
  const expectedSignalMissing =
    expectedSignalKinds.length > 0 && expectedSignalsPresent.length === 0;
  const expectedSignalPresentNotTop =
    expectedSignalsPresent.length > 0 &&
    initialTopActionKind !== null &&
    !expectedSignalSet.has(initialTopActionKind);
  const unexpectedTopAction =
    Boolean(initialTopActionKind) &&
    expectedSignalKinds.length > 0 &&
    !expectedSignalSet.has(initialTopActionKind);

  return {
    expected_signal_missing: expectedSignalMissing,
    expected_signal_present_not_top: expectedSignalPresentNotTop,
    unexpected_top_action: unexpectedTopAction,
    followup_regression_introduced: outcome?.followup_regression_introduced ?? false,
    top_action_cleared: outcome?.top_action_cleared ?? false,
    final_session_clean: outcome?.final_session_clean ?? false,
  };
}

function outcomeBucketForEntry(entry) {
  if (entry.status !== 'completed') {
    return 'provider_failed';
  }
  if (entry.evidence_flags.followup_regression_introduced) {
    return 'regressed';
  }
  if (entry.evidence_flags.expected_signal_missing) {
    return entry.outcome.final_session_clean ? 'clean_but_missed_expected_signal' : 'missed_expected_signal';
  }
  if (entry.evidence_flags.expected_signal_present_not_top) {
    return entry.outcome.final_session_clean
      ? 'clean_but_misranked'
      : 'expected_signal_present_not_top';
  }
  if (entry.outcome.convergence_status === 'thrashing') {
    return 'thrashing';
  }
  if (entry.outcome.convergence_status === 'stalled') {
    return 'stalled';
  }
  if (entry.outcome.final_session_clean) {
    return 'clean';
  }

  return 'incomplete';
}

function normalizeBatchEntry(entry, lane) {
  const outcome = entry.outcome ?? {};
  const expectedSignalKinds = uniqueSortedStrings(asArray(entry.expected_signal_kinds));
  const initialActionKinds = normalizeInitialActionKinds(outcome);
  const initialTopActionKind =
    typeof outcome.initial_top_action_kind === 'string' ? outcome.initial_top_action_kind : null;
  const tags = uniqueSortedStrings(asArray(entry.tags));
  const normalizedEntry = {
    session_id: sessionIdForEntry(entry),
    session_label: sessionLabelForEntry(entry),
    lane,
    status: entry.status ?? 'completed',
    output_dir: entry.output_dir ?? null,
    tags,
    expected_signal_kinds: expectedSignalKinds,
    expected_fix_surface: entry.expected_fix_surface ?? null,
    experiment_arm: normalizeExperimentArm(entry.experiment_arm),
    session_goal: entry.session_goal ?? null,
    success_criteria: entry.success_criteria ?? null,
    focus_areas: focusAreasForExpectedSignals(expectedSignalKinds, tags),
    outcome: {
      session_count: outcome.session_count ?? null,
      initial_action_kinds: initialActionKinds,
      initial_top_action_kind: initialTopActionKind,
      top_action_cleared: outcome.top_action_cleared ?? false,
      checks_to_clear_top_action: outcome.checks_to_clear_top_action ?? null,
      convergence_status: outcome.convergence_status ?? null,
      entropy_delta: outcome.entropy_delta ?? null,
      final_gate: outcome.final_gate ?? null,
      final_session_clean: outcome.final_session_clean ?? false,
      followup_regression_introduced: outcome.followup_regression_introduced ?? false,
    },
  };

  normalizedEntry.evidence_flags = buildEvidenceFlags(
    normalizedEntry.expected_signal_kinds,
    normalizedEntry.outcome.initial_action_kinds,
    normalizedEntry.outcome.initial_top_action_kind,
    normalizedEntry.outcome,
  );
  normalizedEntry.outcome_bucket = outcomeBucketForEntry(normalizedEntry);

  return normalizedEntry;
}

function normalizeBatchEntries(batch, lane) {
  const results = asArray(batch?.results).map(function fromResult(result) {
    return normalizeBatchEntry(result, lane);
  });
  const failures = asArray(batch?.failures).map(function fromFailure(failure) {
    return normalizeBatchEntry(failure, lane);
  });

  return [...results, ...failures];
}

function countEntries(entries, predicate) {
  return entries.filter(predicate).length;
}

function countFocusArea(entries, focusArea) {
  return countEntries(entries, function hasFocusArea(entry) {
    return entry.focus_areas.includes(focusArea);
  });
}

function hasOutcomeBucket(entry, outcomeBucket) {
  return entry.outcome_bucket === outcomeBucket;
}

function hasAnyOutcomeBucket(entry, outcomeBuckets) {
  return outcomeBuckets.has(entry.outcome_bucket);
}

function isExpectedSignalEscapeBucket(outcomeBucket) {
  return (
    MISSED_EXPECTED_SIGNAL_BUCKETS.has(outcomeBucket) ||
    EXPECTED_SIGNAL_NOT_TOP_BUCKETS.has(outcomeBucket)
  );
}

function countFocusAreaEscapes(entries, focusArea) {
  return countEntries(entries, function hasFocusAreaEscape(entry) {
    return (
      entry.focus_areas.includes(focusArea) &&
      isExpectedSignalEscapeBucket(entry.outcome_bucket)
    );
  });
}

function hasTopActionSession(entry) {
  return (
    typeof entry.outcome.initial_top_action_kind === 'string' &&
    entry.outcome.initial_top_action_kind.length > 0
  );
}

function hasClearedTopAction(entry) {
  return hasTopActionSession(entry) && entry.outcome.top_action_cleared === true;
}

function hasRegressionAfterFix(entry) {
  return hasClearedTopAction(entry) && entry.outcome.followup_regression_introduced === true;
}

function isMissedExpectedSignalBucket(entry) {
  return hasAnyOutcomeBucket(entry, MISSED_EXPECTED_SIGNAL_BUCKETS);
}

function isExpectedSignalPresentNotTopBucket(entry) {
  return hasAnyOutcomeBucket(entry, EXPECTED_SIGNAL_NOT_TOP_BUCKETS);
}

function focusAreaCountsForEntries(entries) {
  const counts = new Map();

  for (const entry of entries) {
    for (const focusArea of entry.focus_areas) {
      counts.set(focusArea, (counts.get(focusArea) ?? 0) + 1);
    }
  }

  return [...counts.entries()]
    .map(function toFocusAreaCountEntry([focus_area, session_count]) {
      return {
        focus_area,
        session_count,
      };
    })
    .sort(function compareFocusAreaCounts(left, right) {
      if (right.session_count !== left.session_count) {
        return right.session_count - left.session_count;
      }

      return left.focus_area.localeCompare(right.focus_area);
    });
}

function buildFocusAreaSummary(entries, focusArea) {
  const focusAreaEntries = entries.filter(function hasFocusArea(entry) {
    return entry.focus_areas.includes(focusArea);
  });
  const sessionCount = focusAreaEntries.length;
  const topActionSessionCount = countEntries(focusAreaEntries, hasTopActionSession);
  const topActionClearedCount = countEntries(focusAreaEntries, hasClearedTopAction);
  const regressionAfterFixCount = countEntries(focusAreaEntries, hasRegressionAfterFix);
  const missedExpectedSignalCount = countEntries(focusAreaEntries, isMissedExpectedSignalBucket);
  const expectedSignalPresentNotTopCount = countEntries(
    focusAreaEntries,
    isExpectedSignalPresentNotTopBucket,
  );

  return {
    focus_area: focusArea,
    session_count: sessionCount,
    review_queue_count: countEntries(focusAreaEntries, needsReview),
    top_action_session_count: topActionSessionCount,
    top_action_cleared_count: topActionClearedCount,
    regression_after_fix_count: regressionAfterFixCount,
    missed_expected_signal_count: missedExpectedSignalCount,
    expected_signal_present_not_top_count: expectedSignalPresentNotTopCount,
    regressed_count: countEntries(focusAreaEntries, function isRegressed(entry) {
      return hasOutcomeBucket(entry, 'regressed');
    }),
    thrashing_count: countEntries(focusAreaEntries, function isThrashing(entry) {
      return hasOutcomeBucket(entry, 'thrashing');
    }),
    stalled_count: countEntries(focusAreaEntries, function isStalled(entry) {
      return hasOutcomeBucket(entry, 'stalled');
    }),
    provider_failure_count: countEntries(focusAreaEntries, function isProviderFailed(entry) {
      return hasOutcomeBucket(entry, 'provider_failed');
    }),
    agent_clear_rate: safeRatio(topActionClearedCount, topActionSessionCount),
    review_queue_rate: safeRatio(countEntries(focusAreaEntries, needsReview), sessionCount),
    escape_rate: safeRatio(
      missedExpectedSignalCount + expectedSignalPresentNotTopCount,
      sessionCount,
    ),
  };
}

export function buildFocusAreaSummaries(entries) {
  return focusAreaCountsForEntries(entries).map(function toFocusAreaSummary(entry) {
    return buildFocusAreaSummary(entries, entry.focus_area);
  });
}

export function buildTopActionFailureSummary(entries) {
  return TOP_ACTION_FAILURE_BUCKETS
    .map(function toFailureSummary(outcomeBucket) {
      const bucketEntries = entries.filter(function hasBucket(entry) {
        return hasOutcomeBucket(entry, outcomeBucket);
      });

      if (bucketEntries.length === 0) {
        return null;
      }

      return {
        outcome_bucket: outcomeBucket,
        session_count: bucketEntries.length,
        focus_area_counts: focusAreaCountsForEntries(bucketEntries),
        top_action_session_count: countEntries(bucketEntries, hasTopActionSession),
        top_action_cleared_count: countEntries(bucketEntries, hasClearedTopAction),
        regression_after_fix_count: countEntries(bucketEntries, hasRegressionAfterFix),
        review_queue_count: countEntries(bucketEntries, needsReview),
      };
    })
    .filter(Boolean)
    .sort(function compareFailureSummaries(left, right) {
      if (right.session_count !== left.session_count) {
        return right.session_count - left.session_count;
      }

      return left.outcome_bucket.localeCompare(right.outcome_bucket);
    });
}

export function buildExperimentArmSummaries(entries) {
  const arms = new Map();

  for (const entry of entries) {
    const arm = normalizeExperimentArm(entry.experiment_arm);
    if (!arm) {
      continue;
    }

    if (!arms.has(arm)) {
      arms.set(arm, {
        experiment_arm: arm,
        session_count: 0,
        clean_session_count: 0,
        regression_session_count: 0,
        review_queue_count: 0,
        top_action_session_count: 0,
        top_action_cleared_count: 0,
        regression_after_fix_count: 0,
        focus_area_counts: new Map(),
      });
    }

    const armEntry = arms.get(arm);
    armEntry.session_count += 1;
    if (entry.outcome?.final_session_clean) {
      armEntry.clean_session_count += 1;
    }
    if (entry.outcome?.followup_regression_introduced) {
      armEntry.regression_session_count += 1;
    }
    if (needsReview(entry)) {
      armEntry.review_queue_count += 1;
    }
    if (hasTopActionSession(entry)) {
      armEntry.top_action_session_count += 1;
      if (hasClearedTopAction(entry)) {
        armEntry.top_action_cleared_count += 1;
        if (hasRegressionAfterFix(entry)) {
          armEntry.regression_after_fix_count += 1;
        }
      }
    }

    for (const focusArea of entry.focus_areas) {
      armEntry.focus_area_counts.set(
        focusArea,
        (armEntry.focus_area_counts.get(focusArea) ?? 0) + 1,
      );
    }
  }

  return [...arms.values()]
    .map(function finalizeArm(entry) {
      return {
        experiment_arm: entry.experiment_arm,
        session_count: entry.session_count,
        clean_session_count: entry.clean_session_count,
        regression_session_count: entry.regression_session_count,
        review_queue_count: entry.review_queue_count,
        top_action_session_count: entry.top_action_session_count,
        top_action_cleared_count: entry.top_action_cleared_count,
        regression_after_fix_count: entry.regression_after_fix_count,
        focus_area_counts: [...entry.focus_area_counts.entries()]
          .map(function toFocusAreaCountEntry([focus_area, session_count]) {
            return {
              focus_area,
              session_count,
            };
          })
          .sort(function compareFocusAreaCounts(left, right) {
            if (right.session_count !== left.session_count) {
              return right.session_count - left.session_count;
            }

            return left.focus_area.localeCompare(right.focus_area);
          }),
        clean_rate: safeRatio(entry.clean_session_count, entry.session_count),
        agent_clear_rate: safeRatio(entry.top_action_cleared_count, entry.top_action_session_count),
        review_queue_rate: safeRatio(entry.review_queue_count, entry.session_count),
        regression_rate: safeRatio(entry.regression_session_count, entry.session_count),
        regression_after_fix_rate: safeRatio(
          entry.regression_after_fix_count,
          entry.top_action_session_count,
        ),
      };
    })
    .sort(function compareExperimentArms(left, right) {
      return left.experiment_arm.localeCompare(right.experiment_arm);
    });
}

function buildCorpusSummary(entries, sessionTelemetry, focusAreaSummaries, topActionFailureSummary, experimentArmSummaries) {
  const liveEntries = entries.filter(function isLive(entry) {
    return entry.lane === 'live';
  });
  const replayEntries = entries.filter(function isReplay(entry) {
    return entry.lane === 'replay';
  });
  const cleanSessionCount = countEntries(entries, function isClean(entry) {
    return entry.outcome.final_session_clean;
  });
  const providerFailureCount = countEntries(entries, function isProviderFailure(entry) {
    return entry.outcome_bucket === 'provider_failed';
  });
  const regressionSessionCount = countEntries(entries, function isRegression(entry) {
    return hasOutcomeBucket(entry, 'regressed');
  });
  const topActionSessionCount = countEntries(entries, hasTopActionSession);
  const topActionClearedCount = countEntries(entries, hasClearedTopAction);
  const regressionAfterFixCount = countEntries(entries, hasRegressionAfterFix);
  const thrashingSessionCount = countEntries(entries, function isThrashing(entry) {
    return hasOutcomeBucket(entry, 'thrashing');
  });
  const stalledSessionCount = countEntries(entries, function isStalled(entry) {
    return hasOutcomeBucket(entry, 'stalled');
  });
  const missedExpectedSignalCount = countEntries(entries, isMissedExpectedSignalBucket);
  const misrankedExpectedSignalCount = countEntries(
    entries,
    isExpectedSignalPresentNotTopBucket,
  );

  const propagationSessionCount = countFocusArea(entries, 'propagation');
  const cloneSessionCount = countFocusArea(entries, 'clone_followthrough');

  return {
    session_count: entries.length,
    live_session_count: liveEntries.length,
    replay_session_count: replayEntries.length,
    clean_session_count: cleanSessionCount,
    provider_failure_count: providerFailureCount,
    regression_session_count: regressionSessionCount,
    top_action_session_count: topActionSessionCount,
    top_action_cleared_count: topActionClearedCount,
    regression_after_fix_count: regressionAfterFixCount,
    thrashing_session_count: thrashingSessionCount,
    stalled_session_count: stalledSessionCount,
    missed_expected_signal_count: missedExpectedSignalCount,
    expected_signal_present_not_top_count: misrankedExpectedSignalCount,
    propagation_session_count: propagationSessionCount,
    clone_session_count: cloneSessionCount,
    focus_area_count: focusAreaSummaries.length,
    top_action_failure_count: topActionFailureSummary.length,
    experiment_arm_count: experimentArmSummaries.length,
    agent_clear_rate: safeRatio(topActionClearedCount, topActionSessionCount),
    provider_failure_rate: safeRatio(providerFailureCount, entries.length),
    regression_after_fix_rate: safeRatio(regressionAfterFixCount, topActionSessionCount),
    propagation_escape_rate: safeRatio(countFocusAreaEscapes(entries, 'propagation'), propagationSessionCount),
    duplicate_logic_introduced_rate: safeRatio(
      countEntries(entries, function hasSessionClone(entry) {
        return entry.outcome.initial_action_kinds.includes('session_introduced_clone');
      }),
      entries.length,
    ),
    clone_followthrough_escape_rate: safeRatio(
      countFocusAreaEscapes(entries, 'clone_followthrough'),
      cloneSessionCount,
    ),
    telemetry_session_count: sessionTelemetry?.summary?.session_count ?? 0,
    telemetry_thrashing_session_count:
      sessionTelemetry?.summary?.thrashing_session_count ?? 0,
    telemetry_average_entropy_delta:
      sessionTelemetry?.summary?.average_entropy_delta ?? null,
  };
}

function needsReview(entry) {
  return (
    entry.outcome_bucket === 'regressed' ||
    entry.outcome_bucket === 'thrashing' ||
    entry.outcome_bucket === 'stalled' ||
    isExpectedSignalEscapeBucket(entry.outcome_bucket)
  );
}

export function selectReviewQueue(entries) {
  return entries
    .filter(needsReview)
    .sort(function compareEntries(left, right) {
      if (left.lane !== right.lane) {
        return left.lane.localeCompare(right.lane);
      }
      return left.session_id.localeCompare(right.session_id);
    });
}

export function buildSessionCorpus({
  repoLabel = null,
  repoRoot = null,
  sessionTelemetry = null,
  codexBatch = null,
  replayBatch = null,
}) {
  const entries = [
    ...normalizeBatchEntries(codexBatch, 'live'),
    ...normalizeBatchEntries(replayBatch, 'replay'),
  ].sort(function compareEntries(left, right) {
    if (left.lane !== right.lane) {
      return left.lane.localeCompare(right.lane);
    }
    return left.session_id.localeCompare(right.session_id);
  });
  const focusAreaSummaries = buildFocusAreaSummaries(entries);
  const topActionFailureSummary = buildTopActionFailureSummary(entries);
  const experimentArmSummaries = buildExperimentArmSummaries(entries);

  return {
    schema_version: 1,
    generated_at: new Date().toISOString(),
    repo_label:
      repoLabel ??
      codexBatch?.repo_label ??
      replayBatch?.repo_label ??
      sessionTelemetry?.repo_label ??
      null,
    repo_root:
      repoRoot ??
      codexBatch?.repo_root ??
      replayBatch?.repo_root ??
      sessionTelemetry?.repo_root ??
      null,
    summary: buildCorpusSummary(
      entries,
      sessionTelemetry,
      focusAreaSummaries,
      topActionFailureSummary,
      experimentArmSummaries,
    ),
    focus_area_summaries: focusAreaSummaries,
    top_action_failure_summary: topActionFailureSummary,
    experiment_arm_summaries: experimentArmSummaries,
    review_queue: selectReviewQueue(entries),
    sessions: entries,
  };
}

function appendSessionSection(lines, title, entries) {
  if (entries.length === 0) {
    return;
  }

  lines.push(`## ${title}`);
  lines.push('');
  for (const entry of entries) {
    lines.push(
      `- [${entry.lane}] \`${entry.session_id}\`: bucket=${entry.outcome_bucket}, expected=[${entry.expected_signal_kinds.join(', ')}], top=${entry.outcome.initial_top_action_kind ?? 'none'}, clean=${entry.outcome.final_session_clean}, regression=${entry.outcome.followup_regression_introduced}`,
    );
  }
  lines.push('');
}

function appendFocusAreaSection(lines, focusAreaSummaries) {
  if (focusAreaSummaries.length === 0) {
    return;
  }

  lines.push('## Focus Areas');
  lines.push('');
  for (const entry of focusAreaSummaries) {
    lines.push(
      `- \`${entry.focus_area}\`: sessions=${entry.session_count}, review=${entry.review_queue_count}, clear=${entry.top_action_cleared_count}, miss=${entry.missed_expected_signal_count}, misrank=${entry.expected_signal_present_not_top_count}, escape=${entry.escape_rate ?? 'n/a'}`,
    );
  }
  lines.push('');
}

function appendFailureSection(lines, failureSummaries) {
  if (failureSummaries.length === 0) {
    return;
  }

  lines.push('## Top Action Failures');
  lines.push('');
  for (const entry of failureSummaries) {
    const focusAreaSummary = focusAreaCountsToText(entry.focus_area_counts);
    lines.push(
      `- \`${entry.outcome_bucket}\`: sessions=${entry.session_count}, review=${entry.review_queue_count}, focus=[${focusAreaSummary}]`,
    );
  }
  lines.push('');
}

function appendExperimentArmSection(lines, experimentArms) {
  if (experimentArms.length === 0) {
    return;
  }

  lines.push('## Experiment Arms');
  lines.push('');
  for (const entry of experimentArms) {
    const focusAreaSummary = focusAreaCountsToText(entry.focus_area_counts);
    lines.push(
      `- \`${entry.experiment_arm}\`: sessions=${entry.session_count}, clear=${entry.agent_clear_rate ?? 'n/a'}, clean=${entry.clean_rate ?? 'n/a'}, regressions=${entry.regression_rate ?? 'n/a'}, review=${entry.review_queue_rate ?? 'n/a'}, focus=[${focusAreaSummary}]`,
    );
  }
  lines.push('');
}

function focusAreaCountsToText(focusAreaCounts) {
  const counts = asArray(focusAreaCounts);
  if (counts.length === 0) {
    return 'none';
  }

  return counts
    .map(function formatFocusAreaCount(entry) {
      return `${entry.focus_area}:${entry.session_count}`;
    })
    .join(', ');
}

export function formatSessionCorpusMarkdown(corpus) {
  const lines = [];
  lines.push('# Session Corpus');
  lines.push('');
  lines.push(`- repo: \`${corpus.repo_label ?? 'unknown'}\``);
  lines.push(`- repo root: \`${corpus.repo_root ?? 'unknown'}\``);
  lines.push(`- generated at: \`${corpus.generated_at}\``);
  lines.push(`- sessions: ${corpus.summary.session_count}`);
  lines.push(`- live sessions: ${corpus.summary.live_session_count}`);
  lines.push(`- replay sessions: ${corpus.summary.replay_session_count}`);
  lines.push(`- top-action sessions: ${corpus.summary.top_action_session_count ?? 0}`);
  lines.push(`- top actions cleared: ${corpus.summary.top_action_cleared_count ?? 0}`);
  lines.push(`- agent clear rate: ${corpus.summary.agent_clear_rate ?? 'n/a'}`);
  lines.push(`- focus areas tracked: ${corpus.summary.focus_area_count ?? 0}`);
  lines.push(`- top action failures tracked: ${corpus.summary.top_action_failure_count ?? 0}`);
  lines.push(`- experiment arms tracked: ${corpus.summary.experiment_arm_count ?? 0}`);
  lines.push(
    `- regression-after-fix rate: ${corpus.summary.regression_after_fix_rate ?? 'n/a'}`,
  );
  lines.push(`- propagation escape rate: ${corpus.summary.propagation_escape_rate ?? 'n/a'}`);
  lines.push(
    `- duplicate logic introduced rate: ${corpus.summary.duplicate_logic_introduced_rate ?? 'n/a'}`,
  );
  lines.push(
    `- clone followthrough escape rate: ${corpus.summary.clone_followthrough_escape_rate ?? 'n/a'}`,
  );
  lines.push('');

  appendFocusAreaSection(lines, asArray(corpus.focus_area_summaries).slice(0, 10));
  appendFailureSection(lines, asArray(corpus.top_action_failure_summary).slice(0, 10));
  appendExperimentArmSection(lines, asArray(corpus.experiment_arm_summaries).slice(0, 10));
  appendSessionSection(lines, 'Review Queue', corpus.review_queue.slice(0, 10));
  appendSessionSection(
    lines,
    'Propagation Examples',
    corpus.sessions
      .filter(function isPropagation(entry) {
        return entry.focus_areas.includes('propagation');
      })
      .slice(0, 10),
  );
  appendSessionSection(
    lines,
    'Clone Examples',
    corpus.sessions
      .filter(function isClone(entry) {
        return entry.focus_areas.includes('clone_followthrough');
      })
      .slice(0, 10),
  );

  return `${lines.join('\n')}\n`;
}
