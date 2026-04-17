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
    dry_run: false,
    output_dir: options.outputDir,
    started_at: startedAt,
    finished_at: finishedAt,
    duration_ms: durationMs,
    scenarios: scenarios.map(buildRunScenarioEntry),
    tasks: taskResults,
    summary: summarizeTaskResults(taskResults),
  };
}

export { buildRunIndex, buildTaskResultSummary, summarizeTaskResults };
