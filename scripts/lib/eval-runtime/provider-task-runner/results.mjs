import { nowIso } from '../common.mjs';

function buildTaskResultSummary(item, resultPath, result) {
  return {
    scenario_id: item.scenario.scenario_id,
    task_id: item.task.task_id,
    kind: item.task.kind,
    mode: item.task.mode ?? null,
    result_path: resultPath,
    status: result.evaluation.status,
    score_0_100: result.evaluation.score_0_100,
    adjudication_verdict: result.adjudication?.decision?.verdict ?? null,
    adjudication_structured_evidence_only:
      result.adjudication?.conservative_guardrails?.structured_evidence_only ?? null,
    adjudication_auto_apply_eligible:
      result.adjudication?.conservative_guardrails?.auto_apply_eligible ?? null,
    adjudication_requires_human_review:
      result.adjudication?.conservative_guardrails?.requires_human_review ?? null,
  };
}

function summarizeTaskResults(taskResults) {
  return {
    task_count: taskResults.length,
    pass_count: taskResults.filter((task) => task.status === 'pass').length,
    warn_count: taskResults.filter((task) => task.status === 'warn').length,
    fail_count: taskResults.filter((task) => task.status === 'fail').length,
    dry_run_count: taskResults.filter((task) => task.status === 'dry_run').length,
  };
}

function countMatching(tasks, predicate) {
  return tasks.filter(predicate).length;
}

function buildAdjudicationSummary(taskResults) {
  const adjudicationTasks = taskResults.filter(function isAdjudicationTask(task) {
    return task.kind === 'bounded_adjudication';
  });
  if (adjudicationTasks.length === 0) {
    return null;
  }

  return {
    task_count: adjudicationTasks.length,
    pass_count: countMatching(adjudicationTasks, (task) => task.status === 'pass'),
    warn_count: countMatching(adjudicationTasks, (task) => task.status === 'warn'),
    fail_count: countMatching(adjudicationTasks, (task) => task.status === 'fail'),
    structured_evidence_only_count: countMatching(
      adjudicationTasks,
      (task) => task.adjudication_structured_evidence_only === true,
    ),
    auto_apply_disabled_count: countMatching(
      adjudicationTasks,
      (task) => task.adjudication_auto_apply_eligible === false,
    ),
    human_review_required_count: countMatching(
      adjudicationTasks,
      (task) => task.adjudication_requires_human_review === true,
    ),
    decision_counts: {
      keep: countMatching(
        adjudicationTasks,
        (task) => task.adjudication_verdict === 'keep',
      ),
      rerank_lower: countMatching(
        adjudicationTasks,
        (task) => task.adjudication_verdict === 'rerank_lower',
      ),
      suppress: countMatching(
        adjudicationTasks,
        (task) => task.adjudication_verdict === 'suppress',
      ),
      needs_human_review: countMatching(
        adjudicationTasks,
        (task) => task.adjudication_verdict === 'needs_human_review',
      ),
    },
  };
}

function buildRunIndex({
  runId,
  options,
  scenarios,
  taskResults,
  startedAt,
  durationMs,
  buildRunScenarioEntry,
}) {
  const finishedAt = nowIso();
  return {
    schema_version: 1,
    generated_at: finishedAt,
    run_id: runId,
    provider: options.provider,
    model: options.model ?? null,
    dry_run: options.dryRun === true,
    output_dir: options.outputDir,
    started_at: startedAt,
    finished_at: finishedAt,
    duration_ms: durationMs,
    scenarios: scenarios.map(buildRunScenarioEntry),
    tasks: taskResults,
    summary: summarizeTaskResults(taskResults),
    bounded_adjudication: buildAdjudicationSummary(taskResults),
  };
}

export { buildRunIndex, buildTaskResultSummary, summarizeTaskResults };
