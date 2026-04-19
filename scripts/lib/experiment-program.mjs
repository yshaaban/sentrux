import { existsSync } from 'node:fs';
import path from 'node:path';

import { nowIso, readJson, resolveManifestPath, writeJson } from './eval-batch.mjs';
import { runNodeScript } from './repo-calibration-loop-support.mjs';
import { loadRepoCalibrationManifest } from './repo-calibration-loop/manifest.mjs';

const EXPERIMENT_STATUSES = new Set([
  'planned',
  'in_progress',
  'completed',
  'blocked',
]);
const EXPERIMENT_WORKSTREAMS = new Set([
  'default_lane',
  'structural_pressure',
  'obligation_breadth',
  'treatment_baseline',
  'bounded_llm_adjudication',
]);
const VARIANT_STATUSES = new Set([
  'active',
  'screening',
  'shortlisted',
  'selected',
  'rejected',
]);
const EXPERIMENT_STAGE_IDS = new Set([
  'screen',
  'confirm',
  'decide',
]);
const RUN_EXECUTION_MODES = new Set([
  'repo_calibration_loop',
  'manual_review',
  'policy_branch',
]);
const RUN_FLAGS = [
  ['skip_live', '--skip-live'],
  ['skip_replay', '--skip-replay'],
  ['skip_review', '--skip-review'],
  ['skip_scorecard', '--skip-scorecard'],
  ['skip_backlog', '--skip-backlog'],
];
const SUMMARY_METRIC_KEYS = [
  'top_action_follow_rate',
  'top_action_help_rate',
  'task_success_rate',
  'patch_expansion_rate',
  'intervention_net_value_score',
  'ranking_miss_count',
  'promotion_candidate_count',
  'demotion_candidate_count',
  'reviewer_disagreement_rate',
  'repair_packet_complete_rate',
  'remediation_success_rate',
];
const DEFAULT_LANE_SOURCES = new Set([
  'obligation',
  'rules',
  'clone',
  'structural',
]);
const DEFAULT_LANE_KIND_RULE_KEYS = new Set([
  'eligible',
  'require_patch_directly_worsened',
  'require_repair_surface',
  'require_changed_scope',
]);
const EXPECTED_RUN_ARTIFACTS = {
  repo_calibration_loop: {
    artifact_key: 'summary_json',
    skip_flag: null,
  },
  evidence_review: {
    artifact_key: 'evidence_review_json',
    skip_flag: 'skip_backlog',
  },
  session_corpus: {
    artifact_key: 'session_corpus_json',
    skip_flag: null,
  },
  scorecard: {
    artifact_key: 'scorecard_json',
    skip_flag: 'skip_scorecard',
  },
  backlog: {
    artifact_key: 'backlog_json',
    skip_flag: 'skip_backlog',
  },
};

function assertNonEmptyString(value, label, sourcePath) {
  if (typeof value !== 'string' || value.trim().length === 0) {
    throw new Error(`Invalid ${label} in ${sourcePath}`);
  }

  return value;
}

function assertArray(value, label, sourcePath) {
  if (!Array.isArray(value)) {
    throw new Error(`Invalid ${label} in ${sourcePath}`);
  }

  return value;
}

function assertEnum(value, allowed, label, sourcePath) {
  if (!allowed.has(value)) {
    throw new Error(`Invalid ${label} "${value}" in ${sourcePath}`);
  }

  return value;
}

function normalizeBooleanFlags(flags = {}) {
  const normalized = {};
  for (const [flagName] of RUN_FLAGS) {
    normalized[flagName] = Boolean(flags?.[flagName]);
  }

  return normalized;
}

function validateArtifactExpectation(expectation, specPath, runId) {
  if (typeof expectation !== 'string' || !(expectation in EXPECTED_RUN_ARTIFACTS)) {
    throw new Error(
      `Invalid artifact expectation "${expectation}" in run "${runId}" (${specPath})`,
    );
  }
}

function validateDefaultLaneKindRule(rule, specPath, variantId, kind) {
  if (!rule || typeof rule !== 'object') {
    throw new Error(
      `Invalid default-lane kind rule for "${kind}" in variant "${variantId}" (${specPath})`,
    );
  }

  for (const [key, value] of Object.entries(rule)) {
    if (!DEFAULT_LANE_KIND_RULE_KEYS.has(key)) {
      throw new Error(
        `Unknown default_lane.kind_rules.${kind}.${key} in variant "${variantId}" (${specPath})`,
      );
    }
    if (typeof value !== 'boolean') {
      throw new Error(
        `Invalid default_lane.kind_rules.${kind}.${key} in variant "${variantId}" (${specPath})`,
      );
    }
  }
}

function validateDefaultLaneOverride(defaultLane, specPath, variantId) {
  if (!defaultLane || typeof defaultLane !== 'object') {
    throw new Error(
      `Invalid default_lane override in variant "${variantId}" (${specPath})`,
    );
  }

  if (defaultLane.max_primary_actions !== undefined) {
    const value = Number(defaultLane.max_primary_actions);
    if (!Number.isInteger(value) || value < 1) {
      throw new Error(
        `Invalid max_primary_actions in variant "${variantId}" (${specPath})`,
      );
    }
  }
  if (defaultLane.eligible_sources !== undefined) {
    const eligibleSources = assertArray(
      defaultLane.eligible_sources,
      'default_lane.eligible_sources',
      specPath,
    );
    for (const source of eligibleSources) {
      assertEnum(source, DEFAULT_LANE_SOURCES, 'eligible source', specPath);
    }
  }
  if (defaultLane.kind_rules === undefined) {
    return;
  }
  if (!defaultLane.kind_rules || typeof defaultLane.kind_rules !== 'object') {
    throw new Error(
      `Invalid default_lane.kind_rules in variant "${variantId}" (${specPath})`,
    );
  }

  for (const [kind, rule] of Object.entries(defaultLane.kind_rules)) {
    validateDefaultLaneKindRule(rule, specPath, variantId, kind);
  }
}

function validateVariantPolicyOverride(policyOverride, specPath, variantId) {
  if (!policyOverride || typeof policyOverride !== 'object') {
    throw new Error(`Invalid policy_override in variant "${variantId}" (${specPath})`);
  }

  if (policyOverride.default_lane === undefined) {
    return;
  }

  validateDefaultLaneOverride(policyOverride.default_lane, specPath, variantId);
}

function validateCompletedExperimentDecision(spec, specPath) {
  if (spec.status !== 'completed') {
    return;
  }
  if (!spec.decision || !spec.decision.outcome) {
    throw new Error(`Completed experiment spec is missing decision in ${specPath}`);
  }
  if (!spec.decision_record_path) {
    throw new Error(`Completed experiment spec is missing decision_record_path in ${specPath}`);
  }

  const decisionRecordPath = resolveManifestPath(specPath, spec.decision_record_path);
  if (!existsSync(decisionRecordPath)) {
    throw new Error(
      `Completed experiment spec is missing decision record file ${decisionRecordPath}`,
    );
  }
}

function validateVariant(variant, specPath) {
  if (!variant || typeof variant !== 'object') {
    throw new Error(`Invalid variant entry in ${specPath}`);
  }

  assertNonEmptyString(variant.variant_id, 'variant_id', specPath);
  assertNonEmptyString(variant.name, 'variant name', specPath);
  assertNonEmptyString(variant.description, 'variant description', specPath);

  if (variant.status !== undefined) {
    assertEnum(variant.status, VARIANT_STATUSES, 'variant status', specPath);
  }
  if (variant.policy_override !== undefined) {
    validateVariantPolicyOverride(variant.policy_override, specPath, variant.variant_id);
  }
}

function validateStages(stages, specPath) {
  const stageIds = new Set();
  for (const stage of stages) {
    if (!stage || typeof stage !== 'object') {
      throw new Error(`Invalid stage entry in ${specPath}`);
    }

    assertEnum(stage.stage_id, EXPERIMENT_STAGE_IDS, 'stage_id', specPath);
    assertNonEmptyString(stage.title, 'stage title', specPath);
    assertEnum(stage.status, EXPERIMENT_STATUSES, 'stage status', specPath);
    const exitBar = assertArray(stage.exit_bar, 'stage exit_bar', specPath);
    if (exitBar.length === 0) {
      throw new Error(`Stage "${stage.stage_id}" has empty exit_bar in ${specPath}`);
    }
    if (stage.notes !== undefined && stage.notes !== null) {
      assertNonEmptyString(stage.notes, 'stage notes', specPath);
    }
    if (stageIds.has(stage.stage_id)) {
      throw new Error(`Duplicate stage_id "${stage.stage_id}" in ${specPath}`);
    }
    stageIds.add(stage.stage_id);
  }
}

function validateRepoRun(run, specPath, variantIds) {
  if (!run || typeof run !== 'object') {
    throw new Error(`Invalid repo_run entry in ${specPath}`);
  }

  assertNonEmptyString(run.run_id, 'run_id', specPath);
  assertNonEmptyString(run.repo_id, 'repo_id', specPath);
  assertNonEmptyString(run.variant_id, 'variant_id', specPath);
  if (!variantIds.has(run.variant_id)) {
    throw new Error(
      `Unknown variant_id "${run.variant_id}" referenced by run "${run.run_id}" in ${specPath}`,
    );
  }
  assertNonEmptyString(run.manifest, 'run manifest', specPath);
  assertNonEmptyString(run.output_dir, 'run output_dir', specPath);

  if (run.execution_mode !== undefined) {
    assertEnum(run.execution_mode, RUN_EXECUTION_MODES, 'run execution_mode', specPath);
  }
  if (run.artifact_expectations !== undefined) {
    const expectations = assertArray(
      run.artifact_expectations,
      'run artifact_expectations',
      specPath,
    );
    for (const expectation of expectations) {
      validateArtifactExpectation(expectation, specPath, run.run_id);
    }
  }
}

function validateDecision(decision, specPath, variantIds) {
  if (decision === null || decision === undefined) {
    return;
  }

  if (!decision || typeof decision !== 'object') {
    throw new Error(`Invalid decision block in ${specPath}`);
  }

  const allowed = new Set([
    'promote',
    'keep_experimental',
    'demote',
    'discard',
    'expand_follow_up',
  ]);
  assertEnum(decision.outcome, allowed, 'decision outcome', specPath);
  if (decision.winner_variant_id !== undefined && decision.winner_variant_id !== null) {
    if (!variantIds.has(decision.winner_variant_id)) {
      throw new Error(
        `Unknown winner_variant_id "${decision.winner_variant_id}" in ${specPath}`,
      );
    }
  }
}

function validateAutomatedRunVariants(repoRuns, variantMap, specPath) {
  const seen = new Map();

  for (const run of repoRuns) {
    const executionMode = run.execution_mode ?? 'repo_calibration_loop';
    if (executionMode !== 'repo_calibration_loop') {
      continue;
    }

    const variant = variantMap.get(run.variant_id);
    const key = JSON.stringify({
      repo_id: run.repo_id,
      manifest: run.manifest,
      flags: normalizeBooleanFlags(run.flags),
      policy_override: variant?.policy_override ?? null,
    });
    const previous = seen.get(key);
    if (previous && previous.variant_id !== run.variant_id) {
      throw new Error(
        `Repo-calibration runs "${previous.run_id}" and "${run.run_id}" in ${specPath} differ only by variant label/output without a distinct policy override`,
      );
    }
    seen.set(key, {
      run_id: run.run_id,
      variant_id: run.variant_id,
    });
  }
}

function validateExperimentSpec(spec, specPath) {
  if (spec?.schema_version !== 1) {
    throw new Error(`Unsupported experiment spec: ${specPath}`);
  }

  assertNonEmptyString(spec.experiment_id, 'experiment_id', specPath);
  assertNonEmptyString(spec.title, 'title', specPath);
  assertEnum(spec.workstream, EXPERIMENT_WORKSTREAMS, 'workstream', specPath);
  assertEnum(spec.status, EXPERIMENT_STATUSES, 'status', specPath);
  assertNonEmptyString(spec.cycle_id, 'cycle_id', specPath);
  assertNonEmptyString(spec.program_id, 'program_id', specPath);
  assertNonEmptyString(spec.phase_id, 'phase_id', specPath);
  assertNonEmptyString(spec.owner_doc, 'owner_doc', specPath);
  assertNonEmptyString(spec.decision_question, 'decision_question', specPath);
  assertNonEmptyString(spec.hypothesis, 'hypothesis', specPath);
  assertArray(spec.primary_metrics, 'primary_metrics', specPath);
  assertArray(spec.secondary_metrics, 'secondary_metrics', specPath);
  assertArray(spec.exit_bar, 'exit_bar', specPath);
  if (spec.question_id !== undefined) {
    assertNonEmptyString(spec.question_id, 'question_id', specPath);
  }
  if (spec.decision_record_path !== undefined) {
    assertNonEmptyString(spec.decision_record_path, 'decision_record_path', specPath);
  }
  if (spec.repo_scope !== undefined) {
    const repoScope = assertArray(spec.repo_scope, 'repo_scope', specPath);
    if (repoScope.length === 0) {
      throw new Error(`Experiment spec has empty repo_scope: ${specPath}`);
    }
    for (const repoId of repoScope) {
      assertNonEmptyString(repoId, 'repo_scope entry', specPath);
    }
  }

  const variants = assertArray(spec.variants, 'variants', specPath);
  if (variants.length === 0) {
    throw new Error(`Experiment spec has no variants: ${specPath}`);
  }
  const variantIds = new Set();
  for (const variant of variants) {
    validateVariant(variant, specPath);
    if (variantIds.has(variant.variant_id)) {
      throw new Error(`Duplicate variant_id "${variant.variant_id}" in ${specPath}`);
    }
    variantIds.add(variant.variant_id);
  }
  if (spec.control_variant_id !== undefined) {
    assertNonEmptyString(spec.control_variant_id, 'control_variant_id', specPath);
    if (!variantIds.has(spec.control_variant_id)) {
      throw new Error(
        `Unknown control_variant_id "${spec.control_variant_id}" in ${specPath}`,
      );
    }
  }
  if (spec.stages !== undefined) {
    validateStages(assertArray(spec.stages, 'stages', specPath), specPath);
  }
  const variantMap = buildVariantMap(spec.variants);

  const repoRuns = assertArray(spec.repo_runs, 'repo_runs', specPath);
  const runIds = new Set();
  for (const run of repoRuns) {
    validateRepoRun(run, specPath, variantIds);
    if (runIds.has(run.run_id)) {
      throw new Error(`Duplicate run_id "${run.run_id}" in ${specPath}`);
    }
    runIds.add(run.run_id);
  }
  validateAutomatedRunVariants(repoRuns, variantMap, specPath);

  validateDecision(spec.decision, specPath, variantIds);
  validateCompletedExperimentDecision(spec, specPath);
}

function validateExperimentIndex(index, indexPath) {
  if (index?.schema_version !== 1) {
    throw new Error(`Unsupported experiment index: ${indexPath}`);
  }

  const experiments = assertArray(index.experiments, 'experiments', indexPath);
  const experimentIds = new Set();
  for (const experiment of experiments) {
    if (!experiment || typeof experiment !== 'object') {
      throw new Error(`Invalid experiment reference in ${indexPath}`);
    }
    assertNonEmptyString(experiment.experiment_id, 'experiment reference id', indexPath);
    assertNonEmptyString(experiment.path, 'experiment reference path', indexPath);
    if (experimentIds.has(experiment.experiment_id)) {
      throw new Error(
        `Duplicate experiment reference id "${experiment.experiment_id}" in ${indexPath}`,
      );
    }
    experimentIds.add(experiment.experiment_id);
  }
}

function normalizeFilters(values) {
  if (!values || values.length === 0) {
    return null;
  }

  return new Set(values);
}

function matchesFilter(value, filters) {
  if (!filters) {
    return true;
  }

  return filters.has(value);
}

function buildRepoCalibrationArgs(manifestPath, outputDir, flags = {}) {
  const args = ['--manifest', manifestPath, '--output-dir', outputDir];
  for (const [flagName, cliFlag] of RUN_FLAGS) {
    if (flags?.[flagName]) {
      args.push(cliFlag);
    }
  }

  return args;
}

function shellQuote(value) {
  if (/^[A-Za-z0-9_./:@=-]+$/.test(value)) {
    return value;
  }

  return JSON.stringify(value);
}

function buildCommandString(command, args, env = {}) {
  const parts = [];
  const envArgs = [];
  for (const [key, value] of Object.entries(env)) {
    envArgs.push(`${key}=${shellQuote(value)}`);
  }
  if (envArgs.length > 0) {
    parts.push('env', ...envArgs);
  }

  parts.push(shellQuote(command), ...args.map(shellQuote));
  return parts.join(' ');
}

function countAutomatedRuns(runs) {
  let automatedRunCount = 0;
  for (const run of runs) {
    if (run.execution_mode === 'repo_calibration_loop') {
      automatedRunCount += 1;
    }
  }

  return automatedRunCount;
}

function selectExistingPath(...candidatePaths) {
  for (const candidatePath of candidatePaths) {
    if (candidatePath && existsSync(candidatePath)) {
      return candidatePath;
    }
  }

  return null;
}

function buildVariantMap(variants) {
  const variantMap = new Map();
  for (const variant of variants) {
    variantMap.set(variant.variant_id, variant);
  }

  return variantMap;
}

function pickSummaryMetrics(summary) {
  const metrics = {};
  for (const key of SUMMARY_METRIC_KEYS) {
    if (summary?.summary?.[key] !== undefined) {
      metrics[key] = summary.summary[key];
    }
  }

  if (summary?.summary?.default_on_ready !== undefined) {
    metrics.default_on_ready = summary.summary.default_on_ready;
  }
  if (summary?.summary?.default_on_repo_treatment_ready !== undefined) {
    metrics.default_on_repo_treatment_ready =
      summary.summary.default_on_repo_treatment_ready;
  }
  if (summary?.summary?.bounded_adjudication_status !== undefined) {
    metrics.bounded_adjudication_status =
      summary.summary.bounded_adjudication_status;
  }

  return metrics;
}

function resolveRunOutputDir(targetRepoRootPath, outputDir) {
  if (path.isAbsolute(outputDir)) {
    return outputDir;
  }

  return path.resolve(targetRepoRootPath, outputDir);
}

function resolveExpectationStatus(expectations, artifacts, flags = {}) {
  const satisfied = [];
  const missing = [];
  const skipped = [];

  for (const expectation of expectations) {
    const descriptor = EXPECTED_RUN_ARTIFACTS[expectation];
    if (!descriptor) {
      missing.push(expectation);
      continue;
    }

    if (descriptor.skip_flag && flags?.[descriptor.skip_flag]) {
      skipped.push(expectation);
      continue;
    }

    if (artifacts[descriptor.artifact_key]) {
      satisfied.push(expectation);
      continue;
    }

    missing.push(expectation);
  }

  return {
    satisfied,
    missing,
    skipped,
  };
}

function artifactStateForSummary(artifacts, outputDir, expectations, flags = {}) {
  const expectationStatus = resolveExpectationStatus(expectations, artifacts, flags);
  if (expectations.length > 0) {
    if (expectationStatus.missing.length === 0) {
      return {
        artifact_state: 'completed',
        ...expectationStatus,
      };
    }
    if (
      expectationStatus.satisfied.length > 0 ||
      expectationStatus.skipped.length > 0 ||
      existsSync(outputDir)
    ) {
      return {
        artifact_state: 'partial',
        ...expectationStatus,
      };
    }

    return {
      artifact_state: 'not_started',
      ...expectationStatus,
    };
  }

  if (artifacts.summary_json) {
    return {
      artifact_state: 'completed',
      ...expectationStatus,
    };
  }
  if (existsSync(outputDir)) {
    return {
      artifact_state: 'partial',
      ...expectationStatus,
    };
  }

  return {
    artifact_state: 'not_started',
    ...expectationStatus,
  };
}

function buildStatusCounts() {
  return {
    planned: 0,
    in_progress: 0,
    completed: 0,
    blocked: 0,
  };
}

function buildRunArtifactCounts() {
  return {
    completed: 0,
    partial: 0,
    not_started: 0,
    failed: 0,
    skipped: 0,
  };
}

function maxIso(values) {
  const filtered = values.filter(Boolean).sort();
  return filtered.length === 0 ? null : filtered[filtered.length - 1];
}

function resolveRunCommand(executionMode, runnerScriptPath, args, env = {}) {
  if (executionMode !== 'repo_calibration_loop') {
    return null;
  }

  return buildCommandString('node', [runnerScriptPath, ...args], env);
}

function resolveRunArtifacts(summary, outputDir) {
  const summaryPath = path.join(outputDir, 'repo-calibration-loop.json');
  const summaryMarkdownPath = path.join(outputDir, 'repo-calibration-loop.md');
  const evidenceReviewPath = path.join(outputDir, 'evidence-review.json');
  const sessionCorpusPath = path.join(outputDir, 'session-corpus.json');
  const scorecardPath = path.join(outputDir, 'signal-scorecard.json');
  const backlogPath = path.join(outputDir, 'signal-backlog.json');

  return {
    summary_json: selectExistingPath(summaryPath),
    summary_markdown: selectExistingPath(summaryMarkdownPath),
    evidence_review_json: selectExistingPath(
      summary?.artifacts?.evidence_review_json,
      evidenceReviewPath,
    ),
    session_corpus_json: selectExistingPath(
      summary?.artifacts?.session_corpus_json,
      sessionCorpusPath,
    ),
    scorecard_json: selectExistingPath(
      summary?.artifacts?.scorecard_json,
      scorecardPath,
    ),
    backlog_json: selectExistingPath(
      summary?.artifacts?.backlog_json,
      backlogPath,
    ),
  };
}

function cloneJson(value) {
  return JSON.parse(JSON.stringify(value));
}

function mergeSignalPolicy(basePolicy, policyOverride) {
  const mergedPolicy = cloneJson(basePolicy);
  const defaultLaneOverride = policyOverride?.default_lane;
  if (!defaultLaneOverride) {
    return mergedPolicy;
  }

  const baseDefaultLane = mergedPolicy.default_lane ?? {};
  const mergedDefaultLane = {
    ...baseDefaultLane,
  };
  if (defaultLaneOverride.max_primary_actions !== undefined) {
    mergedDefaultLane.max_primary_actions = defaultLaneOverride.max_primary_actions;
  }
  if (defaultLaneOverride.eligible_sources !== undefined) {
    mergedDefaultLane.eligible_sources = defaultLaneOverride.eligible_sources;
  }
  if (defaultLaneOverride.kind_rules !== undefined) {
    mergedDefaultLane.kind_rules = {
      ...(baseDefaultLane.kind_rules ?? {}),
      ...defaultLaneOverride.kind_rules,
    };
  }

  mergedPolicy.default_lane = mergedDefaultLane;
  return mergedPolicy;
}

function buildPolicyOverrideContext(executionMode, outputDir, policyBasePath, variant) {
  if (executionMode !== 'repo_calibration_loop' || !variant?.policy_override) {
    return {
      env: {},
      policy_base_path: null,
      policy_override: variant?.policy_override ?? null,
      policy_override_path: null,
    };
  }

  const policyOverridePath = path.join(outputDir, 'variant-signal-policy.json');
  return {
    env: {
      SENTRUX_SIGNAL_POLICY_PATH: policyOverridePath,
    },
    policy_base_path: policyBasePath,
    policy_override: variant.policy_override,
    policy_override_path: policyOverridePath,
  };
}

function normalizeArtifactExpectations(artifactExpectations) {
  return Array.isArray(artifactExpectations) ? artifactExpectations : [];
}

function buildPlannedRun({
  executionMode,
  manifest,
  manifestPath,
  normalizedFlags,
  outputDir,
  policyBasePath,
  run,
  runnerScriptPath,
  targetRepoRootPath,
  variantMap,
}) {
  const variant = variantMap.get(run.variant_id);
  const args =
    executionMode === 'repo_calibration_loop'
      ? buildRepoCalibrationArgs(manifestPath, outputDir, normalizedFlags)
      : [];
  const policyOverrideContext = buildPolicyOverrideContext(
    executionMode,
    outputDir,
    policyBasePath,
    variant,
  );

  return {
    run_id: run.run_id,
    repo_id: run.repo_id,
    repo_label: manifest.repo_label ?? manifest.repo_id ?? run.repo_id,
    variant_id: run.variant_id,
    variant_name: variant?.name ?? run.variant_id,
    execution_mode: executionMode,
    manifest_path: manifestPath,
    repo_root_path: targetRepoRootPath,
    output_dir: outputDir,
    flags: normalizedFlags,
    notes: run.notes ?? null,
    artifact_expectations: normalizeArtifactExpectations(run.artifact_expectations),
    ...policyOverrideContext,
    args,
    command: resolveRunCommand(
      executionMode,
      runnerScriptPath,
      args,
      policyOverrideContext.env,
    ),
  };
}

function findRunState(runStates, runId) {
  for (const state of runStates) {
    if (state.run_id === runId) {
      return state;
    }
  }

  return null;
}

function buildTrackerRun(run, state) {
  return {
    run_id: run.run_id,
    repo_id: run.repo_id,
    repo_label: run.repo_label,
    variant_id: run.variant_id,
    variant_name: run.variant_name,
    execution_mode: run.execution_mode,
    output_dir: run.output_dir,
    command: run.command,
    artifact_expectations: run.artifact_expectations,
    artifact_state: state?.artifact_state ?? 'not_started',
    generated_at: state?.generated_at ?? null,
    metrics: state?.metrics ?? {},
    artifacts: state?.artifacts ?? {},
    policy_override_path: run.policy_override_path ?? null,
    missing_artifacts: state?.missing_artifacts ?? [],
    satisfied_artifacts: state?.satisfied_artifacts ?? [],
    skipped_artifacts: state?.skipped_artifacts ?? [],
  };
}

function formatMetrics(metrics) {
  return Object.entries(metrics)
    .map(function formatMetricEntry([key, value]) {
      return `${key}=${formatMetricValue(value)}`;
    })
    .join(', ');
}

function appendMetricSummary(lines, metrics, prefix) {
  if (Object.keys(metrics).length === 0) {
    return;
  }

  lines.push(`${prefix}${formatMetrics(metrics)}`);
}

function collectGeneratedAtValues(runStates) {
  const generatedAtValues = [];
  for (const state of runStates) {
    generatedAtValues.push(state.generated_at);
  }

  return generatedAtValues;
}

function deriveExperimentNextGate(spec, runStatusCounts, runCount, decisionRecordPresent) {
  if (runStatusCounts.completed < runCount) {
    return 'fresh_runs_required';
  }
  if (spec.status === 'completed' && spec.decision?.outcome && decisionRecordPresent) {
    return 'decision_recorded';
  }
  if (!spec.decision?.outcome) {
    return 'decision_review_required';
  }

  return 'decision_record_required';
}

function normalizeStages(stages = []) {
  const normalizedStages = [];
  for (const stage of stages) {
    normalizedStages.push({
      stage_id: stage.stage_id,
      title: stage.title,
      status: stage.status,
      exit_bar: stage.exit_bar,
      notes: stage.notes ?? null,
    });
  }

  return normalizedStages;
}

function buildActiveStage(stages) {
  for (const stage of stages) {
    if (stage.status !== 'completed') {
      return {
        stage_id: stage.stage_id,
        title: stage.title,
        status: stage.status,
      };
    }
  }

  return null;
}

function deriveExecutionStatus(runResults) {
  for (const result of runResults) {
    if (result.status === 'failed') {
      return 'partial_failure';
    }
  }

  return 'completed';
}

export async function loadExperimentSpec(specPath) {
  const spec = await readJson(specPath);
  validateExperimentSpec(spec, specPath);
  return spec;
}

export async function loadExperimentIndex(indexPath) {
  const index = await readJson(indexPath);
  validateExperimentIndex(index, indexPath);
  return index;
}

export async function loadExperimentRegistry(indexPath) {
  const index = await loadExperimentIndex(indexPath);
  const experiments = [];

  for (const experiment of index.experiments) {
    const specPath = resolveManifestPath(indexPath, experiment.path);
    const spec = await loadExperimentSpec(specPath);
    if (spec.experiment_id !== experiment.experiment_id) {
      throw new Error(
        `Experiment reference "${experiment.experiment_id}" does not match spec id "${spec.experiment_id}" in ${specPath}`,
      );
    }
    experiments.push({
      experiment_id: experiment.experiment_id,
      spec_path: specPath,
      spec,
    });
  }

  return {
    index,
    experiments,
  };
}

export async function buildExperimentRunPlan({
  specPath,
  repoRootPath,
  repoIds = null,
  runIds = null,
  variantIds = null,
}) {
  const spec = await loadExperimentSpec(specPath);
  const runnerScriptPath = path.join(
    repoRootPath,
    'scripts',
    'evals',
    'run-repo-calibration-loop.mjs',
  );
  const repoFilter = normalizeFilters(repoIds);
  const runFilter = normalizeFilters(runIds);
  const variantFilter = normalizeFilters(variantIds);
  const variantMap = buildVariantMap(spec.variants);
  const policyBasePath = path.join(repoRootPath, '.sentrux', 'signal-policy.json');
  const runs = [];

  for (const run of spec.repo_runs) {
    if (!matchesFilter(run.repo_id, repoFilter)) {
      continue;
    }
    if (!matchesFilter(run.run_id, runFilter)) {
      continue;
    }
    if (!matchesFilter(run.variant_id, variantFilter)) {
      continue;
    }

    const manifestPath = resolveManifestPath(specPath, run.manifest);
    const manifest = await loadRepoCalibrationManifest(manifestPath);
    if (manifest.repo_id && manifest.repo_id !== run.repo_id) {
      throw new Error(
        `Run "${run.run_id}" expects repo_id "${run.repo_id}" but manifest ${manifestPath} is "${manifest.repo_id}"`,
      );
    }
    const targetRepoRootPath = resolveManifestPath(manifestPath, manifest.repo_root);
    const executionMode = run.execution_mode ?? 'repo_calibration_loop';
    const normalizedFlags = normalizeBooleanFlags(run.flags);
    const outputDir = resolveRunOutputDir(targetRepoRootPath, run.output_dir);
    runs.push(
      buildPlannedRun({
        executionMode,
        manifest,
        manifestPath,
        normalizedFlags,
        outputDir,
        policyBasePath,
        run,
        runnerScriptPath,
        targetRepoRootPath,
        variantMap,
      }),
    );
  }

  return {
    schema_version: 1,
    generated_at: nowIso(),
    repo_root_path: repoRootPath,
    spec_path: specPath,
    runner_script_path: runnerScriptPath,
    spec,
    runs,
  };
}

export async function collectExperimentRunState(run) {
  const summaryPath = path.join(run.output_dir, 'repo-calibration-loop.json');
  const summary = existsSync(summaryPath) ? await readJson(summaryPath) : null;
  const artifacts = resolveRunArtifacts(summary, run.output_dir);
  const expectationStatus = artifactStateForSummary(
    artifacts,
    run.output_dir,
    run.artifact_expectations ?? [],
    run.flags,
  );

  return {
    run_id: run.run_id,
    repo_id: run.repo_id,
    variant_id: run.variant_id,
    execution_mode: run.execution_mode,
    output_dir: run.output_dir,
    artifact_state: expectationStatus.artifact_state,
    generated_at: summary?.generated_at ?? null,
    artifacts,
    metrics: pickSummaryMetrics(summary),
    satisfied_artifacts: expectationStatus.satisfied,
    missing_artifacts: expectationStatus.missing,
    skipped_artifacts: expectationStatus.skipped,
  };
}

async function writeRunPolicyOverride(run) {
  if (!run.policy_override || !run.policy_override_path) {
    return;
  }
  if (!run.policy_base_path || !existsSync(run.policy_base_path)) {
    throw new Error(
      `Cannot build variant policy override for run "${run.run_id}" without base policy ${run.policy_base_path ?? '(missing)'}`,
    );
  }

  const basePolicy = await readJson(run.policy_base_path);
  const mergedPolicy = mergeSignalPolicy(basePolicy, run.policy_override);
  await writeJson(run.policy_override_path, mergedPolicy);
}

export async function executeExperimentPlan(plan, { continueOnError = false } = {}) {
  const runResults = [];

  for (const run of plan.runs) {
    const startedAt = nowIso();

    if (run.execution_mode !== 'repo_calibration_loop') {
      runResults.push({
        run_id: run.run_id,
        repo_id: run.repo_id,
        variant_id: run.variant_id,
        execution_mode: run.execution_mode,
        status: 'skipped',
        started_at: startedAt,
        finished_at: nowIso(),
        message: `Run is tracked as ${run.execution_mode} and must be executed outside the automated loop.`,
        command: run.command,
        output_dir: run.output_dir,
        policy_override_path: run.policy_override_path,
        state: await collectExperimentRunState(run),
      });
      continue;
    }

    try {
      await writeRunPolicyOverride(run);
      const execution = await runNodeScript(
        run.repo_root_path,
        plan.runner_script_path,
        run.args,
        { env: run.env },
      );
      runResults.push({
        run_id: run.run_id,
        repo_id: run.repo_id,
        variant_id: run.variant_id,
        execution_mode: run.execution_mode,
        status: 'completed',
        started_at: startedAt,
        finished_at: nowIso(),
        execution,
        command: run.command,
        output_dir: run.output_dir,
        policy_override_path: run.policy_override_path,
        state: await collectExperimentRunState(run),
      });
    } catch (error) {
      const failure = {
        run_id: run.run_id,
        repo_id: run.repo_id,
        variant_id: run.variant_id,
        execution_mode: run.execution_mode,
        status: 'failed',
        started_at: startedAt,
        finished_at: nowIso(),
        error_message: error instanceof Error ? error.message : String(error),
        command: run.command,
        output_dir: run.output_dir,
        policy_override_path: run.policy_override_path,
        state: await collectExperimentRunState(run),
      };
      runResults.push(failure);
      if (!continueOnError) {
        return {
          schema_version: 1,
          generated_at: nowIso(),
          experiment_id: plan.spec.experiment_id,
          spec_path: plan.spec_path,
          status: 'failed',
          run_results: runResults,
        };
      }
    }
  }

  return {
    schema_version: 1,
    generated_at: nowIso(),
    experiment_id: plan.spec.experiment_id,
    spec_path: plan.spec_path,
    status: deriveExecutionStatus(runResults),
    run_results: runResults,
  };
}

export async function buildExperimentTracker({ indexPath, repoRootPath }) {
  const registry = await loadExperimentRegistry(indexPath);
  const statusCounts = buildStatusCounts();
  const stageStatusCounts = buildStatusCounts();
  const experiments = [];
  let totalRuns = 0;
  let totalAutomatedRuns = 0;

  for (const experiment of registry.experiments) {
    const plan = await buildExperimentRunPlan({
      specPath: experiment.spec_path,
      repoRootPath,
    });
    const runStates = [];
    for (const run of plan.runs) {
      runStates.push(await collectExperimentRunState(run));
    }

    statusCounts[plan.spec.status] += 1;
    totalRuns += plan.runs.length;
    totalAutomatedRuns += countAutomatedRuns(plan.runs);
    const stages = normalizeStages(plan.spec.stages);
    for (const stage of stages) {
      stageStatusCounts[stage.status] += 1;
    }

    const runStatusCounts = buildRunArtifactCounts();
    for (const state of runStates) {
      if (runStatusCounts[state.artifact_state] !== undefined) {
        runStatusCounts[state.artifact_state] += 1;
      }
    }

    const latestEvidenceAt = maxIso(collectGeneratedAtValues(runStates));
    const decisionRecordPath = plan.spec.decision_record_path
      ? resolveManifestPath(plan.spec_path, plan.spec.decision_record_path)
      : null;
    const decisionRecordPresent = decisionRecordPath ? existsSync(decisionRecordPath) : false;
    experiments.push({
      experiment_id: plan.spec.experiment_id,
      title: plan.spec.title,
      workstream: plan.spec.workstream,
      status: plan.spec.status,
      cycle_id: plan.spec.cycle_id,
      program_id: plan.spec.program_id,
      question_id: plan.spec.question_id ?? null,
      phase_id: plan.spec.phase_id,
      owner_doc: plan.spec.owner_doc,
      decision_question: plan.spec.decision_question,
      control_variant_id: plan.spec.control_variant_id ?? null,
      repo_scope: plan.spec.repo_scope ?? [],
      primary_metrics: plan.spec.primary_metrics,
      secondary_metrics: plan.spec.secondary_metrics,
      exit_bar: plan.spec.exit_bar,
      decision: plan.spec.decision ?? null,
      decision_record_path: decisionRecordPath,
      decision_record_present: decisionRecordPresent,
      stages,
      active_stage: buildActiveStage(stages),
      run_status_counts: runStatusCounts,
      total_runs: plan.runs.length,
      automated_runs: countAutomatedRuns(plan.runs),
      latest_evidence_at: latestEvidenceAt,
      next_gate: deriveExperimentNextGate(
        plan.spec,
        runStatusCounts,
        plan.runs.length,
        decisionRecordPresent,
      ),
      runs: plan.runs.map(function buildTrackerRunEntry(run) {
        const state = findRunState(runStates, run.run_id);
        return buildTrackerRun(run, state);
      }),
    });
  }

  return {
    schema_version: 1,
    generated_at: nowIso(),
    index_path: indexPath,
    repo_root_path: repoRootPath,
    summary: {
      experiment_count: experiments.length,
      total_run_count: totalRuns,
      automated_run_count: totalAutomatedRuns,
      planned_count: statusCounts.planned,
      in_progress_count: statusCounts.in_progress,
      completed_count: statusCounts.completed,
      blocked_count: statusCounts.blocked,
      stage_planned_count: stageStatusCounts.planned,
      stage_in_progress_count: stageStatusCounts.in_progress,
      stage_completed_count: stageStatusCounts.completed,
      stage_blocked_count: stageStatusCounts.blocked,
    },
    experiments,
  };
}

function formatMetricValue(value) {
  if (typeof value === 'number') {
    return Number.isInteger(value) ? String(value) : value.toFixed(3);
  }
  if (typeof value === 'boolean') {
    return value ? 'true' : 'false';
  }
  return value ?? 'n/a';
}

export function formatExperimentTrackerMarkdown(tracker) {
  const lines = [
    '# Experiment Tracker',
    '',
    `Generated: ${tracker.generated_at}`,
    '',
    `- experiments: ${tracker.summary.experiment_count}`,
    `- in progress: ${tracker.summary.in_progress_count}`,
    `- planned: ${tracker.summary.planned_count}`,
    `- completed: ${tracker.summary.completed_count}`,
    `- blocked: ${tracker.summary.blocked_count}`,
    `- total runs: ${tracker.summary.total_run_count}`,
    `- automated runs: ${tracker.summary.automated_run_count}`,
    `- stages in progress: ${tracker.summary.stage_in_progress_count}`,
    `- stages planned: ${tracker.summary.stage_planned_count}`,
    `- stages completed: ${tracker.summary.stage_completed_count}`,
    `- stages blocked: ${tracker.summary.stage_blocked_count}`,
    '',
  ];

  for (const experiment of tracker.experiments) {
    lines.push(`## ${experiment.title}`);
    lines.push('');
    lines.push(`- experiment id: ${experiment.experiment_id}`);
    lines.push(`- workstream: ${experiment.workstream}`);
    lines.push(`- status: ${experiment.status}`);
    if (experiment.question_id) {
      lines.push(`- question id: ${experiment.question_id}`);
    }
    lines.push(`- phase: ${experiment.phase_id}`);
    if (experiment.control_variant_id) {
      lines.push(`- control variant: ${experiment.control_variant_id}`);
    }
    if (experiment.repo_scope.length > 0) {
      lines.push(`- repo scope: ${experiment.repo_scope.join(', ')}`);
    }
    lines.push(`- decision question: ${experiment.decision_question}`);
    lines.push(
      `- run coverage: ${experiment.run_status_counts.completed}/${experiment.total_runs} completed`,
    );
    lines.push(`- next gate: ${experiment.next_gate}`);
    if (experiment.active_stage) {
      lines.push(
        `- active stage: ${experiment.active_stage.stage_id} (${experiment.active_stage.status})`,
      );
    }
    if (experiment.latest_evidence_at) {
      lines.push(`- latest evidence: ${experiment.latest_evidence_at}`);
    }
    if (experiment.decision?.outcome) {
      lines.push(`- decision: ${experiment.decision.outcome}`);
    }
    if (experiment.decision_record_path) {
      lines.push(
        `- decision record: ${experiment.decision_record_present ? 'present' : 'missing'} (${experiment.decision_record_path})`,
      );
    }
    lines.push('');
    if (experiment.stages.length > 0) {
      lines.push('- stages:');
      for (const stage of experiment.stages) {
        lines.push(`  - ${stage.stage_id} (${stage.status}): ${stage.title}`);
      }
      lines.push('');
    }

    for (const run of experiment.runs) {
      lines.push(
        `- ${run.run_id} (${run.repo_id}, ${run.variant_id}, ${run.execution_mode}): ${run.artifact_state}`,
      );
      if (run.missing_artifacts.length > 0) {
        lines.push(`  missing artifacts: ${run.missing_artifacts.join(', ')}`);
      }
      if (run.skipped_artifacts.length > 0) {
        lines.push(`  skipped artifacts: ${run.skipped_artifacts.join(', ')}`);
      }
      appendMetricSummary(lines, run.metrics, '  metrics: ');
    }
    lines.push('');
  }

  return `${lines.join('\n').trimEnd()}\n`;
}

export function formatExperimentRunMarkdown(result, plan) {
  const lines = [
    '# Experiment Run',
    '',
    `Generated: ${result.generated_at}`,
    `Experiment: ${plan.spec.title} (${plan.spec.experiment_id})`,
    `Status: ${result.status}`,
    '',
  ];

  for (const run of result.run_results) {
    lines.push(`## ${run.run_id}`);
    lines.push('');
    lines.push(`- repo: ${run.repo_id}`);
    lines.push(`- variant: ${run.variant_id}`);
    lines.push(`- execution mode: ${run.execution_mode}`);
    lines.push(`- status: ${run.status}`);
    if (run.output_dir) {
      lines.push(`- output dir: ${run.output_dir}`);
    }
    if (run.command) {
      lines.push(`- command: \`${run.command}\``);
    }
    if (run.policy_override_path) {
      lines.push(`- policy override: ${run.policy_override_path}`);
    }
    if (run.state?.artifact_state) {
      lines.push(`- artifact state: ${run.state.artifact_state}`);
    }
    if (run.state?.missing_artifacts?.length > 0) {
      lines.push(`- missing artifacts: ${run.state.missing_artifacts.join(', ')}`);
    }
    if (run.state?.skipped_artifacts?.length > 0) {
      lines.push(`- skipped artifacts: ${run.state.skipped_artifacts.join(', ')}`);
    }
    if (run.state?.generated_at) {
      lines.push(`- evidence captured: ${run.state.generated_at}`);
    }
    if (run.error_message) {
      lines.push(`- error: ${run.error_message}`);
    }
    appendMetricSummary(lines, run.state?.metrics ?? {}, '- metrics: ');
    lines.push('');
  }

  return `${lines.join('\n').trimEnd()}\n`;
}
