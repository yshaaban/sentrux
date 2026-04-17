import { resolveRepoRoot } from '../scenarios.mjs';

const BASE_APPEND_SYSTEM_PROMPT = [
  'You are an external evaluation worker.',
  'Return only the JSON object that matches the schema passed on the command line.',
  'Do not edit files.',
  'If evidence is uncertain, say so directly and lower confidence instead of speculating.',
].join(' ');

const AGENT_BRIEF_OUTPUT_SCHEMA = {
  type: 'object',
  additionalProperties: false,
  required: ['task_kind', 'repo_name', 'mode', 'summary', 'top_signals', 'next_steps', 'confidence_0_1'],
  properties: {
    task_kind: { const: 'agent_brief' },
    repo_name: { type: 'string', minLength: 1 },
    mode: { enum: ['repo_onboarding', 'patch', 'pre_merge'] },
    summary: { type: 'string', minLength: 1 },
    top_signals: {
      type: 'array',
      minItems: 1,
      items: {
        type: 'object',
        additionalProperties: false,
        required: ['title', 'summary'],
        properties: {
          title: { type: 'string', minLength: 1 },
          summary: { type: 'string', minLength: 1 },
          kind: { type: 'string' },
          severity: { type: 'string' },
          evidence: {
            type: 'array',
            items: { type: 'string' },
          },
          paths: {
            type: 'array',
            items: { type: 'string' },
          },
          confidence_0_1: {
            type: 'number',
            minimum: 0,
            maximum: 1,
          },
        },
      },
    },
    next_steps: {
      type: 'array',
      minItems: 1,
      items: { type: 'string' },
    },
    confidence_0_1: {
      type: 'number',
      minimum: 0,
      maximum: 1,
    },
    warnings: {
      type: 'array',
      items: { type: 'string' },
    },
    notes: {
      type: 'array',
      items: { type: 'string' },
    },
  },
};

const DEAD_PRIVATE_OUTPUT_SCHEMA = {
  type: 'object',
  additionalProperties: false,
  required: ['task_kind', 'repo_name', 'summary', 'candidate_clusters', 'confidence_0_1'],
  properties: {
    task_kind: { const: 'dead_private' },
    repo_name: { type: 'string', minLength: 1 },
    summary: { type: 'string', minLength: 1 },
    candidate_clusters: {
      type: 'array',
      minItems: 1,
      items: {
        type: 'object',
        additionalProperties: false,
        required: ['file_path', 'summary', 'evidence', 'confidence_0_1'],
        properties: {
          file_path: { type: 'string', minLength: 1 },
          symbol: { type: 'string' },
          kind: { type: 'string' },
          summary: { type: 'string', minLength: 1 },
          evidence: {
            type: 'array',
            minItems: 1,
            items: { type: 'string' },
          },
          rationale: { type: 'string' },
          confidence_0_1: {
            type: 'number',
            minimum: 0,
            maximum: 1,
          },
          lines: {
            type: 'array',
            items: { type: 'integer' },
          },
        },
      },
    },
    confidence_0_1: {
      type: 'number',
      minimum: 0,
      maximum: 1,
    },
    warnings: {
      type: 'array',
      items: { type: 'string' },
    },
  },
};

function buildOutputSchema(task) {
  if (task.kind === 'agent_brief') {
    return AGENT_BRIEF_OUTPUT_SCHEMA;
  }

  return DEAD_PRIVATE_OUTPUT_SCHEMA;
}

function buildTaskPrompt(scenario, scenarioPath, task) {
  const repoRoot = resolveRepoRoot(scenario, scenarioPath);
  const lines = [
    `Repository: ${scenario.repo.name}`,
    `Repository root: ${repoRoot}`,
    `Task kind: ${task.kind}`,
  ];

  if (task.kind === 'agent_brief') {
    lines.push(`Mode: ${task.mode}`);
  }

  lines.push('');
  lines.push(task.prompt.trim());
  return lines.join('\n');
}

function defaultChecksForTask(task) {
  if (task.kind === 'agent_brief') {
    return [
      { kind: 'has', path: 'task_kind', severity: 'required' },
      { kind: 'enum', path: 'mode', allowed: [task.mode], severity: 'required' },
      { kind: 'has', path: 'summary', severity: 'required' },
      { kind: 'min_items', path: 'top_signals', min: 1, severity: 'required' },
      { kind: 'min_items', path: 'next_steps', min: 1, severity: 'required' },
      { kind: 'has', path: 'confidence_0_1', severity: 'required' },
    ];
  }

  return [
    { kind: 'has', path: 'task_kind', severity: 'required' },
    { kind: 'has', path: 'summary', severity: 'required' },
    { kind: 'min_items', path: 'candidate_clusters', min: 1, severity: 'required' },
    { kind: 'has', path: 'confidence_0_1', severity: 'required' },
  ];
}

export {
  AGENT_BRIEF_OUTPUT_SCHEMA,
  BASE_APPEND_SYSTEM_PROMPT,
  DEAD_PRIVATE_OUTPUT_SCHEMA,
  buildOutputSchema,
  buildTaskPrompt,
  defaultChecksForTask,
};
