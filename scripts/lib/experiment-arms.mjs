const EXPERIMENT_ARM_ALIASES = Object.freeze({
  baseline: 'no_intervention',
  'baseline/no-intervention': 'no_intervention',
  'no-intervention': 'no_intervention',
  no_intervention: 'no_intervention',
  'report-only': 'report_only',
  report_only: 'report_only',
  'fix-first': 'fix_this_first',
  fix_first: 'fix_this_first',
  directive_fix_first: 'fix_this_first',
  'stop-and-refactor': 'stop_and_refactor',
  stop_and_refactor: 'stop_and_refactor',
  directive_stop_and_refactor: 'stop_and_refactor',
});

function normalizeList(values) {
  return [...new Set((values ?? []).filter(Boolean).map(String))];
}

export function normalizeExperimentArm(value) {
  if (typeof value !== 'string') {
    return null;
  }

  const normalizedValue = value.trim();
  if (normalizedValue.length === 0) {
    return null;
  }

  return EXPERIMENT_ARM_ALIASES[normalizedValue] ?? normalizedValue;
}

function buildSharedContextLines({
  sessionGoal,
  successCriteria,
  expectedSignalKinds,
  expectedFixSurface,
}) {
  const lines = [];
  if (typeof sessionGoal === 'string' && sessionGoal.trim().length > 0) {
    lines.push(`- session goal: ${sessionGoal.trim()}`);
  }
  if (typeof successCriteria === 'string' && successCriteria.trim().length > 0) {
    lines.push(`- success criteria: ${successCriteria.trim()}`);
  }

  const normalizedKinds = normalizeList(expectedSignalKinds);
  if (normalizedKinds.length > 0) {
    lines.push(`- expected signal kinds: ${normalizedKinds.join(', ')}`);
  }

  if (typeof expectedFixSurface === 'string' && expectedFixSurface.trim().length > 0) {
    lines.push(`- expected fix surface: ${expectedFixSurface.trim()}`);
  }

  return lines;
}

function buildArmInstructions(experimentArm) {
  switch (experimentArm) {
    case 'report_only':
      return [
        'Use Sentrux outputs as diagnostic context, but do not let them redirect the task unless they directly block correctness.',
        'Prefer completing the requested change over opportunistic cleanup.',
        'If a surfaced issue is not task-critical, record it mentally and keep the edit contained.',
      ];
    case 'fix_this_first':
      return [
        'Before broad edits, inspect the current top Sentrux action and clear it first when it directly blocks a clean change.',
        'Prefer the smallest fix that clears the blocking signal before continuing with the main task.',
        'Do not roam into unrelated cleanup once the blocking action is resolved.',
      ];
    case 'stop_and_refactor':
      return [
        'If the same hotspot resurfaces or the patch keeps expanding, stop patching locally and refactor the root cause first.',
        'Use recurring Sentrux top actions as the stop signal for escalation into a contained refactor.',
        'Resume task-specific edits only after the refactor removes the repeated pressure.',
      ];
    default:
      return [];
  }
}

export function applyExperimentArmToPrompt(prompt, options = {}) {
  const experimentArm = normalizeExperimentArm(options.experimentArm);
  if (!experimentArm || experimentArm === 'no_intervention') {
    return prompt;
  }

  const contextLines = buildSharedContextLines(options);
  const instructionLines = buildArmInstructions(experimentArm);
  if (instructionLines.length === 0) {
    return prompt;
  }

  const lines = [];
  lines.push('Calibration experiment context:');
  lines.push(`- experiment arm: ${experimentArm}`);
  lines.push(...contextLines);
  lines.push('');
  lines.push('Intervention instructions for this run:');
  for (const line of instructionLines) {
    lines.push(`- ${line}`);
  }
  lines.push('');
  lines.push('Original task:');
  lines.push('');
  lines.push(prompt);

  return `${lines.join('\n')}\n`;
}
