import path from 'node:path';
import { existsSync } from 'node:fs';

import { resolveManifestPath } from '../eval-batch.mjs';
import { fail, readJson, resolvePath } from './common.mjs';

function requireScenarioObject(value, message) {
  if (!value || typeof value !== 'object') {
    fail(message);
  }
}

function requireNonEmptyString(value, message) {
  if (typeof value !== 'string' || !value) {
    fail(message);
  }
}

function validateScenarioRepo(repo, scenarioPath) {
  requireScenarioObject(repo, `Scenario missing repo block: ${scenarioPath}`);
  requireNonEmptyString(repo.name, `Scenario missing repo.name: ${scenarioPath}`);
  requireNonEmptyString(repo.root_env, `Scenario missing repo.root_env: ${scenarioPath}`);
  requireNonEmptyString(
    repo.default_root,
    `Scenario missing repo.default_root: ${scenarioPath}`,
  );
}

function validateAgentBriefTask(task, scenarioPath) {
  const supportedModes = new Set(['repo_onboarding', 'patch', 'pre_merge']);
  if (!supportedModes.has(task.mode)) {
    fail(`agent_brief task has unsupported mode in ${scenarioPath}:${task.task_id}`);
  }
}

function validateDeadPrivateTask(task, scenarioPath) {
  if ('mode' in task) {
    fail(`dead_private task must not define mode in ${scenarioPath}:${task.task_id}`);
  }
}

function validateScenarioTask(task, scenarioPath) {
  requireScenarioObject(task, `Scenario task is not an object in ${scenarioPath}`);
  requireNonEmptyString(task.task_id, `Scenario task missing task_id in ${scenarioPath}`);
  requireNonEmptyString(
    task.prompt?.trim() ? task.prompt : '',
    `Scenario task missing prompt in ${scenarioPath}:${task.task_id}`,
  );
  if (task.checks !== undefined && !Array.isArray(task.checks)) {
    fail(`Scenario task checks must be an array in ${scenarioPath}:${task.task_id}`);
  }

  switch (task.kind) {
    case 'agent_brief':
      validateAgentBriefTask(task, scenarioPath);
      return;
    case 'dead_private':
      validateDeadPrivateTask(task, scenarioPath);
      return;
    default:
      fail(`Scenario task has unsupported kind "${task.kind}" in ${scenarioPath}`);
  }
}

export function ensureScenarioShape(scenario, scenarioPath) {
  requireScenarioObject(scenario, `Scenario is not an object: ${scenarioPath}`);
  if (scenario.schema_version !== 1) {
    fail(`Unsupported scenario schema version in ${scenarioPath}: ${scenario.schema_version}`);
  }
  requireNonEmptyString(scenario.scenario_id, `Scenario missing scenario_id: ${scenarioPath}`);
  validateScenarioRepo(scenario.repo, scenarioPath);

  if (!Array.isArray(scenario.tasks) || scenario.tasks.length === 0) {
    fail(`Scenario has no tasks: ${scenarioPath}`);
  }

  for (const task of scenario.tasks) {
    validateScenarioTask(task, scenarioPath);
  }
}

export async function loadScenarioPathsFromManifest(manifestPath) {
  const manifestDir = path.dirname(manifestPath);
  const manifest = await readJson(manifestPath);

  if (!manifest || typeof manifest !== 'object') {
    fail(`Manifest is not an object: ${manifestPath}`);
  }

  if (!Array.isArray(manifest.scenarios) || manifest.scenarios.length === 0) {
    fail(`Manifest has no scenarios: ${manifestPath}`);
  }

  return manifest.scenarios.map((entry) => {
    if (!entry || typeof entry !== 'object') {
      fail(`Invalid scenario entry in manifest: ${manifestPath}`);
    }

    if (typeof entry.path !== 'string' || !entry.path) {
      fail(`Scenario entry missing path in manifest: ${manifestPath}`);
    }

    return resolvePath(manifestDir, entry.path);
  });
}

export function resolveRepoRoot(scenario, scenarioPath) {
  const override = process.env[scenario.repo.root_env];
  if (override) {
    return path.resolve(override);
  }

  return resolveManifestPath(scenarioPath, scenario.repo.default_root);
}

export function assertScenarioRepoExists(repoRoot) {
  if (!existsSync(repoRoot)) {
    fail(`Scenario repo root does not exist: ${repoRoot}`);
  }
}

export function buildScenarioSummary(scenario) {
  return {
    scenario_id: scenario.scenario_id,
    title: scenario.title ?? null,
    description: scenario.description ?? null,
    repo: scenario.repo,
  };
}

export function buildResultPath(outputDir, scenario, task) {
  return path.join(outputDir, scenario.scenario_id, task.task_id, 'result.json');
}

export function buildDryRunScenarioEntry({ scenario, scenarioPath }) {
  return {
    scenario_id: scenario.scenario_id,
    source_path: scenarioPath,
    repo: scenario.repo,
    task_count: scenario.tasks.length,
    tasks: scenario.tasks.map((task) => ({
      task_id: task.task_id,
      kind: task.kind,
      mode: task.mode ?? null,
    })),
  };
}

export function countScenarioTasks(scenarios) {
  return scenarios.reduce((count, scenario) => count + scenario.task_count, 0);
}

export function buildScenarioTaskQueue(scenarios) {
  const allTasks = [];
  for (const entry of scenarios) {
    for (const task of entry.scenario.tasks) {
      allTasks.push({
        scenario: entry.scenario,
        scenarioPath: entry.scenarioPath,
        task,
      });
    }
  }
  return allTasks;
}

export function buildRunScenarioEntry({ scenario, scenarioPath }) {
  return {
    scenario_id: scenario.scenario_id,
    source_path: scenarioPath,
    repo: scenario.repo,
    task_count: scenario.tasks.length,
  };
}

export async function resolveScenarioPaths(options) {
  if (options.scenarioPaths.length > 0) {
    return options.scenarioPaths.map((scenarioPath) => path.resolve(scenarioPath));
  }

  return loadScenarioPathsFromManifest(path.resolve(options.manifestPath));
}

export async function loadScenarioEntries(options) {
  const scenarioPaths = await resolveScenarioPaths(options);
  const scenarios = [];
  for (const scenarioPath of scenarioPaths) {
    const scenario = await readJson(scenarioPath);
    ensureScenarioShape(scenario, scenarioPath);
    scenarios.push({ scenario, scenarioPath });
  }
  return scenarios;
}
