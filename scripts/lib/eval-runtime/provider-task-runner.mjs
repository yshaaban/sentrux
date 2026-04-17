import { assertScenarioRepoExists, buildScenarioSummary, resolveRepoRoot } from './scenarios.mjs';
import { buildTaskPrompt, buildOutputSchema, BASE_APPEND_SYSTEM_PROMPT } from './provider-task-runner/task-schemas.mjs';
import { buildDryRunProviderOutput, runProvider } from './provider-task-runner/provider.mjs';
import { evaluateTask, extractResponsePayload } from './provider-task-runner/evaluation.mjs';
import {
  buildRunIndex,
  buildTaskResultSummary,
  summarizeTaskResults,
} from './provider-task-runner/results.mjs';

export async function runEvalTask({ scenario, scenarioPath, task, options, finishedAt }) {
  const repoRoot = resolveRepoRoot(scenario, scenarioPath);
  assertScenarioRepoExists(repoRoot);

  const providerOutput = options.dryRun
    ? buildDryRunProviderOutput(options, repoRoot)
    : await runProvider({
        provider: options.provider,
        cwd: repoRoot,
        prompt: buildTaskPrompt(scenario, scenarioPath, task),
        model: options.model,
        jsonSchema: buildOutputSchema(task),
        appendSystemPrompt: BASE_APPEND_SYSTEM_PROMPT,
        timeoutMs: options.timeoutMs,
        claudeBin: options.claudeBin,
        codexBin: options.codexBin,
      });

  const responsePayload = extractResponsePayload(providerOutput, task);
  const evaluation = options.dryRun
    ? {
        status: 'dry_run',
        score_0_100: 0,
        required_check_count: 0,
        passed_check_count: 0,
        failed_check_count: 0,
        check_results: [],
        provider_failed: false,
        summary: 'dry run skipped provider execution',
      }
    : evaluateTask(task, responsePayload.response_json, providerOutput);

  return {
    schema_version: 1,
    generated_at: finishedAt,
    run_id: options.runId,
    scenario: {
      source_path: scenarioPath,
      ...buildScenarioSummary(scenario),
    },
    task: {
      task_id: task.task_id,
      kind: task.kind,
      mode: task.mode ?? null,
      prompt: task.prompt,
      notes: task.notes ?? null,
      checks: task.checks ?? [],
    },
    provider: {
      name: providerOutput.provider,
      version: providerOutput.provider_version,
      model: options.model ?? null,
      command: providerOutput.command,
      cwd: providerOutput.cwd,
      timeout_ms: options.timeoutMs,
    },
    execution: {
      started_at: providerOutput.started_at,
      finished_at: finishedAt,
      duration_ms: providerOutput.duration_ms,
      exit_code: providerOutput.exit_code,
      signal: providerOutput.signal,
      timed_out: providerOutput.timed_out,
    },
    response: {
      parse_status: responsePayload.parse_status,
      outer_json: responsePayload.outer_json,
      response_json: responsePayload.response_json,
      response_text: responsePayload.response_text,
      stdout: providerOutput.stdout,
      stderr: providerOutput.stderr,
    },
    evaluation,
  };
}

export { buildRunIndex, buildTaskResultSummary, summarizeTaskResults };
