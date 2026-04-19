import { existsSync } from 'node:fs';
import path from 'node:path';

import { nowIso, readJson, resolveManifestPath } from './eval-batch.mjs';
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

  const repoRuns = assertArray(spec.repo_runs, 'repo_runs', specPath);
  const runIds = new Set();
  for (const run of repoRuns) {
    validateRepoRun(run, specPath, variantIds);
    if (runIds.has(run.run_id)) {
      throw new Error(`Duplicate run_id "${run.run_id}" in ${specPath}`);
    }
    runIds.add(run.run_id);
  }

  validateDecision(spec.decision, specPath, variantIds);
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

function buildCommandString(command, args) {
  return [command, ...args].map(shellQuote).join(' ');
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

function artifactStateForSummary(summaryPath, outputDir) {
  if (summaryPath && existsSync(summaryPath)) {
    return 'completed';
  }
  if (existsSync(outputDir)) {
    return 'partial';
  }

  return 'not_started';
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

function resolveRunCommand(executionMode, runnerScriptPath, args) {
  if (executionMode !== 'repo_calibration_loop') {
    return null;
  }

  return buildCommandString('node', [runnerScriptPath, ...args]);
}

function resolveRunArtifacts(summary, outputDir) {
  const summaryPath = path.join(outputDir, 'repo-calibration-loop.json');
  const summaryMarkdownPath = path.join(outputDir, 'repo-calibration-loop.md');
  const evidenceReviewPath = path.join(outputDir, 'evidence-review.json');
  const sessionCorpusPath = path.join(outputDir, 'session-corpus.json');
  const scorecardPath = path.join(outputDir, 'signal-scorecard.json');

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
  };
}

function buildPlannedRun({
  executionMode,
  manifest,
  manifestPath,
  normalizedFlags,
  outputDir,
  run,
  runnerScriptPath,
  targetRepoRootPath,
  variantMap,
}) {
  let args = [];
  if (executionMode === 'repo_calibration_loop') {
    args = buildRepoCalibrationArgs(manifestPath, outputDir, normalizedFlags);
  }

  return {
    run_id: run.run_id,
    repo_id: run.repo_id,
    repo_label: manifest.repo_label ?? manifest.repo_id ?? run.repo_id,
    variant_id: run.variant_id,
    variant_name: variantMap.get(run.variant_id)?.name ?? run.variant_id,
    execution_mode: executionMode,
    manifest_path: manifestPath,
    repo_root_path: targetRepoRootPath,
    output_dir: outputDir,
    flags: normalizedFlags,
    notes: run.notes ?? null,
    artifact_expectations: Array.isArray(run.artifact_expectations)
      ? run.artifact_expectations
      : [],
    args,
    command: resolveRunCommand(executionMode, runnerScriptPath, args),
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
    artifact_state: state?.artifact_state ?? 'not_started',
    generated_at: state?.generated_at ?? null,
    metrics: state?.metrics ?? {},
    artifacts: state?.artifacts ?? {},
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

function deriveExperimentNextGate(specStatus, runStatusCounts, runCount) {
  if (runStatusCounts.completed < runCount) {
    return 'fresh_runs_required';
  }
  if (specStatus === 'completed') {
    return 'decision_recorded';
  }

  return 'decision_review_required';
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

  return {
    run_id: run.run_id,
    repo_id: run.repo_id,
    variant_id: run.variant_id,
    execution_mode: run.execution_mode,
    output_dir: run.output_dir,
    artifact_state: artifactStateForSummary(artifacts.summary_json, run.output_dir),
    generated_at: summary?.generated_at ?? null,
    artifacts,
    metrics: pickSummaryMetrics(summary),
  };
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
        state: await collectExperimentRunState(run),
      });
      continue;
    }

    try {
      const execution = await runNodeScript(
        run.repo_root_path,
        plan.runner_script_path,
        run.args,
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
      stages,
      active_stage: buildActiveStage(stages),
      run_status_counts: runStatusCounts,
      total_runs: plan.runs.length,
      automated_runs: countAutomatedRuns(plan.runs),
      latest_evidence_at: latestEvidenceAt,
      next_gate: deriveExperimentNextGate(
        plan.spec.status,
        runStatusCounts,
        plan.runs.length,
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
    if (run.state?.artifact_state) {
      lines.push(`- artifact state: ${run.state.artifact_state}`);
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
