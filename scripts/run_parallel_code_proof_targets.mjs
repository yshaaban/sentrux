#!/usr/bin/env node

import { spawnSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';
import {
  assertPathExists,
  createDisposableRepoClone,
} from './lib/disposable-repo.mjs';
import { resolveWorkspaceRepoRoot } from './lib/path-roots.mjs';
import { repoRootFromImportMeta, writeJson } from './lib/script-artifacts.mjs';

const repoRoot = repoRootFromImportMeta(import.meta.url, 1);

const sentruxBin = process.env.SENTRUX_BIN ?? path.join(repoRoot, 'target/debug/sentrux');
const parallelCodeRoot = resolveWorkspaceRepoRoot(
  process.env.PARALLEL_CODE_ROOT,
  'parallel-code',
  repoRoot,
);
const rulesSource =
  process.env.RULES_SOURCE ?? path.join(repoRoot, 'docs/v2/examples/parallel-code.rules.toml');
const outputDir =
  process.env.OUTPUT_DIR ?? path.join(repoRoot, 'docs/v2/examples/parallel-code-proof-runs');

function request(id, name, argumentsObject = {}) {
  return {
    jsonrpc: '2.0',
    id,
    method: 'tools/call',
    params: {
      name,
      arguments: argumentsObject,
    },
  };
}

function runMcpRequests(workRoot, requests) {
  const input = `${requests.map((entry) => JSON.stringify(entry)).join('\n')}\n`;
  const result = spawnSync(sentruxBin, ['mcp'], {
    cwd: repoRoot,
    encoding: 'utf8',
    input,
    maxBuffer: 32 * 1024 * 1024,
  });

  if (result.status !== 0) {
    throw new Error(
      `Sentrux MCP run failed with status ${result.status ?? 'unknown'}:\n${result.stderr}`,
    );
  }

  const responseLines = result.stdout
    .split('\n')
    .map((line) => line.trim())
    .filter((line) => line.startsWith('{'));
  const responses = new Map();
  for (const line of responseLines) {
    const payload = JSON.parse(line);
    responses.set(payload.id, payload);
  }

  return responses;
}

function parseToolResponse(responses, id) {
  const response = responses.get(id);
  if (!response) {
    throw new Error(`Missing MCP response for id ${id}`);
  }

  const text = response.result?.content?.[0]?.text;
  if (typeof text !== 'string') {
    throw new Error(`Missing MCP text payload for id ${id}`);
  }

  return JSON.parse(text);
}

function sanitizeValue(value, workRoot) {
  if (typeof value === 'string') {
    return value.split(workRoot).join(parallelCodeRoot);
  }
  if (Array.isArray(value)) {
    return value.map((entry) => sanitizeValue(entry, workRoot));
  }
  if (value && typeof value === 'object') {
    return Object.fromEntries(
      Object.entries(value).map(([key, entry]) => [key, sanitizeValue(entry, workRoot)]),
    );
  }
  return value;
}

function stabilizeValue(value) {
  if (Array.isArray(value)) {
    return value.map((entry) => stabilizeValue(entry));
  }
  if (value && typeof value === 'object') {
    return Object.fromEntries(
      Object.entries(value)
        .filter(
          ([key]) =>
            key !== 'age_days' &&
            key !== 'youngest_age_days' &&
            key !== 'quality_signal' &&
            key !== 'generated_at',
        )
        .map(([key, entry]) => [key, stabilizeValue(entry)]),
    );
  }
  return value;
}

function normalizedPayload(payload, workRoot) {
  return stabilizeValue(sanitizeValue(payload, workRoot));
}

async function createProofClone(label) {
  return createDisposableRepoClone({
    sourceRoot: parallelCodeRoot,
    label,
    rulesSource,
  });
}

function readFile(targetPath) {
  return readFileSync(targetPath, 'utf8');
}

async function writeMarkdown(targetPath, value) {
  await mkdir(path.dirname(targetPath), { recursive: true });
  await writeFile(targetPath, value, 'utf8');
}

function replaceOnce(source, needle, replacement, label) {
  if (!source.includes(needle)) {
    throw new Error(`Could not find ${label}`);
  }
  return source.replace(needle, replacement);
}

function replaceSpan(source, startNeedle, endNeedle, replacement, label) {
  const startIndex = source.indexOf(startNeedle);
  if (startIndex === -1) {
    throw new Error(`Could not find start of ${label}`);
  }
  const endIndex = source.indexOf(endNeedle, startIndex);
  if (endIndex === -1) {
    throw new Error(`Could not find end of ${label}`);
  }
  return `${source.slice(0, startIndex)}${replacement}${source.slice(endIndex)}`;
}

function sameSortedFiles(expectedFiles, candidateFiles) {
  const sortedExpectedFiles = [...expectedFiles].sort();
  const sortedCandidateFiles = [...candidateFiles].sort();

  return (
    sortedCandidateFiles.length === sortedExpectedFiles.length &&
    sortedCandidateFiles.every((file, index) => file === sortedExpectedFiles[index])
  );
}

function findEntryForFiles(entries, expectedFiles) {
  return (entries ?? []).find((entry) => sameSortedFiles(expectedFiles, entry.files ?? [])) ?? null;
}

function summarizeTaskGitStatusFindings(findingsPayload) {
  return (findingsPayload.findings ?? []).filter(
    (finding) => finding.concept_id === 'task_git_status',
  );
}

function summarizeTaskPresentation(findingsPayload, obligationsPayload) {
  return {
    concept_summary:
      (findingsPayload.concept_summaries ?? []).find(
        (summary) => summary.concept_id === 'task_presentation_status',
      ) ?? null,
    obligations: obligationsPayload.obligations ?? [],
    missing_site_count: obligationsPayload.missing_site_count ?? 0,
  };
}

function summarizeCloneFamily(findingsPayload, expectedFiles) {
  return {
    family: findEntryForFiles(findingsPayload.clone_families, expectedFiles),
    debt_signal: findEntryForFiles(findingsPayload.debt_signals, expectedFiles),
  };
}

function buildOwnershipMarkdown(payload) {
  const beforeCount = payload.before.findings.length;
  const afterCount = payload.after.findings.length;
  const lines = [
    '# Parallel-Code Proof: Ownership Regression',
    '',
    'This scenario seeds an out-of-policy `task_git_status` write from `task-presentation-status.ts`.',
    '',
    `- before task_git_status findings: ${beforeCount}`,
    `- after task_git_status findings: ${afterCount}`,
    `- gate blocked after mutation: ${payload.after.gate.decision === 'fail'}`,
    '',
    '## After Findings',
    '',
  ];

  for (const finding of payload.after.findings) {
    lines.push(`- \`${finding.kind}\`: ${finding.summary}`);
  }

  return `${lines.join('\n')}\n`;
}

function buildPropagationMarkdown(payload) {
  const beforeMissing = payload.before.missing_site_count;
  const afterMissing = payload.after.missing_site_count;
  const lines = [
    '# Parallel-Code Proof: Propagation Cleanup',
    '',
    'This scenario adds an explicit exhaustive `Record<TaskDotStatus, number>` mapping in `task-presentation-status.ts`.',
    '',
    `- before missing sites: ${beforeMissing}`,
    `- after missing sites: ${afterMissing}`,
    '',
    '## Before Summary',
    '',
    payload.before.concept_summary?.summary ?? '- n/a',
    '',
    '## After Summary',
    '',
    payload.after.concept_summary?.summary ??
      'Resolved: the concept no longer appears in the top concept-summary output after the exhaustive record was added.',
  ];

  return `${lines.join('\n')}\n`;
}

function buildCloneMarkdown(payload) {
  const beforeFamily = payload.before.clone.family;
  const afterFamily = payload.after.clone.family;
  const lines = [
    '# Parallel-Code Proof: Clone Family Cleanup',
    '',
    'This scenario extracts shared glyph rendering into a common helper module for `AgentGlyph` and `RemoteAgentGlyph`.',
    '',
    `- before family member count: ${beforeFamily?.member_count ?? 0}`,
    `- after family member count: ${afterFamily?.member_count ?? 0}`,
    '',
    `- before family score: ${beforeFamily?.family_score ?? 'n/a'}`,
    `- after family score: ${afterFamily?.family_score ?? 'n/a'}`,
  ];

  return `${lines.join('\n')}\n`;
}

async function runOwnershipRegressionScenario() {
  const clone = await createProofClone('parallel-code-proof-ownership');

  try {
    const beforeResponses = runMcpRequests(clone.workRoot, [
      request(1, 'scan', { path: clone.workRoot }),
      request(2, 'findings', { limit: 12 }),
      request(3, 'session_start', {}),
    ]);
    const beforeFindings = normalizedPayload(parseToolResponse(beforeResponses, 2), clone.workRoot);

    const statusPath = path.join(clone.workRoot, 'src/app/task-presentation-status.ts');
    const statusSource = readFile(statusPath);
    const statusImported = replaceOnce(
      statusSource,
      "import { store } from '../store/state';\n",
      "import { setStore, store } from '../store/state';\n",
      'task-presentation-status setStore import',
    );
    const injectionPoint = '  const gitStatus = store.taskGitStatus[taskId];\n';
    const mutatedStatus = replaceOnce(
      statusImported,
      injectionPoint,
      `${injectionPoint}  setStore('taskGitStatus', taskId, gitStatus);\n`,
      'task-presentation-status ownership regression injection point',
    );
    await writeFile(statusPath, mutatedStatus, 'utf8');

    const afterResponses = runMcpRequests(clone.workRoot, [
      request(10, 'scan', { path: clone.workRoot }),
      request(11, 'findings', { limit: 12 }),
      request(12, 'gate', {}),
      request(13, 'session_end', {}),
    ]);

    const afterFindings = normalizedPayload(parseToolResponse(afterResponses, 11), clone.workRoot);
    const payload = {
      scenario: 'ownership_regression',
      before: {
        findings: summarizeTaskGitStatusFindings(beforeFindings),
      },
      after: {
        findings: summarizeTaskGitStatusFindings(afterFindings),
        gate: normalizedPayload(parseToolResponse(afterResponses, 12), clone.workRoot),
        session_end: normalizedPayload(parseToolResponse(afterResponses, 13), clone.workRoot),
      },
    };

    return {
      name: 'ownership-regression',
      payload,
      markdown: buildOwnershipMarkdown(payload),
    };
  } finally {
    await clone.cleanup();
  }
}

async function runPropagationCleanupScenario() {
  const clone = await createProofClone('parallel-code-proof-propagation');

  try {
    const beforeResponses = runMcpRequests(clone.workRoot, [
      request(1, 'scan', { path: clone.workRoot }),
      request(2, 'findings', { limit: 12 }),
      request(3, 'obligations', { concept: 'task_presentation_status' }),
    ]);

    const statusPath = path.join(clone.workRoot, 'src/app/task-presentation-status.ts');
    const statusSource = readFile(statusPath);
    const marker = "const TASK_ATTENTION_GROUP_TITLES: Record<TaskAttentionEntry['group'], string> = {\n";
    const exhaustiveMapping = `${marker}  'needs-action': 'Needs Action',\n  ready: 'Ready',\n  quiet: 'Quiet',\n};\n\nconst TASK_DOT_STATUS_DISPLAY_ORDER: Record<TaskDotStatus, number> = {\n  busy: 0,\n  waiting: 1,\n  ready: 2,\n  paused: 3,\n  'flow-controlled': 4,\n  restoring: 5,\n  failed: 6,\n};\n\n`;
    const mutatedStatus = replaceSpan(
      statusSource,
      marker,
      'function getAttentionMetadata(reason: TaskAttentionReason): {\n',
      exhaustiveMapping,
      'TaskDotStatus exhaustive mapping insertion point',
    );
    await writeFile(statusPath, mutatedStatus, 'utf8');

    const afterResponses = runMcpRequests(clone.workRoot, [
      request(10, 'scan', { path: clone.workRoot }),
      request(11, 'findings', { limit: 12 }),
      request(12, 'obligations', { concept: 'task_presentation_status' }),
    ]);

    const beforeFindings = normalizedPayload(parseToolResponse(beforeResponses, 2), clone.workRoot);
    const beforeObligations = normalizedPayload(parseToolResponse(beforeResponses, 3), clone.workRoot);
    const afterFindings = normalizedPayload(parseToolResponse(afterResponses, 11), clone.workRoot);
    const afterObligations = normalizedPayload(parseToolResponse(afterResponses, 12), clone.workRoot);

    const payload = {
      scenario: 'propagation_cleanup',
      before: summarizeTaskPresentation(beforeFindings, beforeObligations),
      after: summarizeTaskPresentation(afterFindings, afterObligations),
    };

    return {
      name: 'propagation-cleanup',
      payload,
      markdown: buildPropagationMarkdown(payload),
    };
  } finally {
    await clone.cleanup();
  }
}

async function runCloneCleanupScenario() {
  const clone = await createProofClone('parallel-code-proof-clone');

  try {
    const beforeResponses = runMcpRequests(clone.workRoot, [
      request(1, 'scan', { path: clone.workRoot }),
      request(2, 'findings', { limit: 12 }),
    ]);

    const sharedRendererPath = path.join(clone.workRoot, 'src/lib/agent-glyph-renderer.tsx');
    const sharedRendererSource = `import type { JSX } from 'solid-js';\n\nexport type SharedAgentGlyphKind =\n  | 'claude'\n  | 'codex'\n  | 'gemini'\n  | 'generic'\n  | 'hydra'\n  | 'opencode';\n\nexport interface SharedGlyphPalette {\n  background: string;\n  border: string;\n  stroke: string;\n  accent: string;\n}\n\nfunction ClaudeGlyph(props: { palette: SharedGlyphPalette }): JSX.Element {\n  return (\n    <svg viewBox=\"0 0 16 16\" fill=\"none\" aria-hidden=\"true\">\n      <path d=\"m3.127 10.604 3.135-1.76.053-.153-.053-.085H6.11l-.525-.032-1.791-.048-1.554-.065-1.505-.08-.38-.081L0 7.832l.036-.234.32-.214.455.04 1.009.069 1.513.105 1.097.064 1.626.17h.259l.036-.105-.089-.065-.068-.064-1.566-1.062-1.695-1.121-.887-.646-.48-.327-.243-.306-.104-.67.435-.48.585.04.15.04.593.456 1.267.981 1.654 1.218.242.202.097-.068.012-.049-.109-.181-.9-1.626-.96-1.655-.428-.686-.113-.411a2 2 0 0 1-.068-.484l.496-.674L4.446 0l.662.089.279.242.411.94.666 1.48 1.033 2.014.302.597.162.553.06.17h.105v-.097l.085-1.134.157-1.392.154-1.792.052-.504.25-.605.497-.327.387.186.319.456-.045.294-.19 1.23-.37 1.93-.243 1.29h.142l.161-.16.654-.868 1.097-1.372.484-.545.565-.601.363-.287h.686l.505.751-.226.775-.707.895-.585.759-.839 1.13-.524.904.048.072.125-.012 1.897-.403 1.024-.186 1.223-.21.553.258.06.263-.218.536-1.307.323-1.533.307-2.284.54-.028.02.032.04 1.029.098.44.024h1.077l2.005.15.525.346.315.424-.053.323-.807.411-3.631-.863-.872-.218h-.12v.073l.726.71 1.331 1.202 1.667 1.55.084.383-.214.302-.226-.032-1.464-1.101-.565-.497-1.28-1.077h-.084v.113l.295.432 1.557 2.34.08.718-.112.234-.404.141-.444-.08-.911-1.28-.94-1.44-.759-1.291-.093.053-.448 4.821-.21.246-.484.186-.403-.307-.214-.496.214-.98.258-1.28.21-1.016.19-1.263.112-.42-.008-.028-.092.012-.953 1.307-1.448 1.957-1.146 1.227-.274.109-.477-.247.045-.44.266-.39 1.586-2.018.956-1.25.617-.723-.004-.105h-.036l-4.212 2.736-.75.096-.324-.302.04-.496.154-.162 1.267-.871z\" fill={props.palette.stroke} />\n    </svg>\n  );\n}\n\nfunction GeminiGlyph(props: { palette: SharedGlyphPalette }): JSX.Element {\n  return (\n    <svg viewBox=\"0 0 16 16\" fill=\"none\" aria-hidden=\"true\">\n      <path d=\"M8 1.5C7.3 5.2 5.2 7.3 1.5 8 5.2 8.7 7.3 10.8 8 14.5 8.7 10.8 10.8 8.7 14.5 8 10.8 7.3 8.7 5.2 8 1.5Z\" fill={props.palette.stroke} />\n      <path d=\"M12.2 2.4C11.9 3.9 11 4.8 9.5 5.1 11 5.4 11.9 6.3 12.2 7.8 12.5 6.3 13.4 5.4 14.9 5.1 13.4 4.8 12.5 3.9 12.2 2.4Z\" fill={props.palette.accent} />\n    </svg>\n  );\n}\n\nfunction CodexGlyph(props: { palette: SharedGlyphPalette }): JSX.Element {\n  return (\n    <svg viewBox=\"0 0 16 16\" fill=\"none\" aria-hidden=\"true\">\n      <path d=\"M14.949 6.547a3.94 3.94 0 0 0-.348-3.273 4.11 4.11 0 0 0-4.4-1.934 4.1 4.1 0 0 0-1.778-.614 4.15 4.15 0 0 0-2.118-.086 4.1 4.1 0 0 0-1.891.948 4.04 4.04 0 0 0-1.158 1.753 4.1 4.1 0 0 0-1.563.679 4 4 0 0 0-1.14 1.253.99.99 0 0 0 .502 4.731 3.94 3.94 0 0 0 .346 3.274 4.11 4.11 0 0 0 4.402 1.933c.382.425.852.764 1.377.995.526.231 1.095.35 1.67.346 1.78.002 3.358-1.132 3.901-2.804a4.1 4.1 0 0 0 1.563-.68 4 4 0 0 0 1.14-1.253 3.99 3.99 0 0 0-.506-4.716m-6.097 8.406a3.05 3.05 0 0 1-1.945-.694l.096-.054 3.23-1.838a.53.53 0 0 0 .265-.455v-4.49l1.366.778q.02.011.025.035v3.722c-.003 1.653-1.361 2.992-3.037 2.996m-6.53-2.75a2.95 2.95 0 0 1-.36-2.01l.095.057L5.29 12.09a.53.53 0 0 0 .527 0l3.949-2.246v1.555a.05.05 0 0 1-.022.041L6.473 13.3c-1.454.826-3.311.335-4.15-1.098m-.85-6.94A3.02 3.02 0 0 1 3.07 3.949v3.785a.51.51 0 0 0 .262.451l3.93 2.237-1.366.779a.05.05 0 0 1-.048 0L2.585 9.342a2.98 2.98 0 0 1-1.113-4.094zm11.216 2.571L8.747 5.576l1.362-.776a.05.05 0 0 1 .048 0l3.265 1.86a3 3 0 0 1 1.173 1.207 2.96 2.96 0 0 1-.27 3.2 3.05 3.05 0 0 1-1.36.997V8.279a.52.52 0 0 0-.276-.445m1.36-2.015-.097-.057-3.226-1.855a.53.53 0 0 0-.53 0L6.249 6.153V4.598a.04.04 0 0 1 .019-.04L9.533 2.7a3.07 3.07 0 0 1 3.257.139c.474.325.843.778 1.066 1.303.223.526.289 1.103.191 1.664zM5.503 8.575 4.139 7.8a.05.05 0 0 1-.026-.037V4.049c0-.57.166-1.127.476-1.607s.752-.864 1.275-1.105a3.08 3.08 0 0 1 3.234.41l-.096.054-3.23 1.838a.53.53 0 0 0-.265.455zm.742-1.577 1.758-1 1.762 1v2l-1.755 1-1.762-1z\" fill={props.palette.stroke} />\n    </svg>\n  );\n}\n\nfunction OpenCodeGlyph(props: { palette: SharedGlyphPalette }): JSX.Element {\n  return (\n    <svg viewBox=\"0 0 16 16\" fill=\"none\" aria-hidden=\"true\">\n      <path d=\"M5.2 5.1 2.9 8l2.3 2.9M10.8 5.1 13.1 8l-2.3 2.9\" stroke={props.palette.stroke} stroke-width=\"1.6\" stroke-linecap=\"round\" stroke-linejoin=\"round\" />\n      <path d=\"M6.9 11.6 9.9 4.4\" stroke={props.palette.accent} stroke-width=\"1.4\" stroke-linecap=\"round\" />\n    </svg>\n  );\n}\n\nfunction HydraGlyph(props: { palette: SharedGlyphPalette }): JSX.Element {\n  return (\n    <svg viewBox=\"0 0 16 16\" fill=\"none\" aria-hidden=\"true\">\n      <path d=\"M8 13V8.9\" stroke={props.palette.accent} stroke-width=\"1.5\" stroke-linecap=\"round\" />\n      <path d=\"M8 9.1 5.1 5.3M8 9.1 10.9 5.3\" stroke={props.palette.stroke} stroke-width=\"1.45\" stroke-linecap=\"round\" />\n      <circle cx=\"8\" cy=\"3.6\" r=\"1.5\" fill={props.palette.accent} />\n      <circle cx=\"4.4\" cy=\"5.1\" r=\"1.5\" fill={props.palette.stroke} opacity=\"0.96\" />\n      <circle cx=\"11.6\" cy=\"5.1\" r=\"1.5\" fill={props.palette.stroke} opacity=\"0.96\" />\n    </svg>\n  );\n}\n\nfunction GenericGlyph(props: { palette: SharedGlyphPalette }): JSX.Element {\n  return (\n    <svg viewBox=\"0 0 16 16\" fill=\"none\" aria-hidden=\"true\">\n      <rect x=\"3.1\" y=\"4.5\" width=\"9.8\" height=\"7\" rx=\"1.4\" stroke={props.palette.stroke} stroke-width=\"1.35\" />\n      <path d=\"m5.1 7 1.5 1.1-1.5 1.1M8.2 9.2h2.6\" stroke={props.palette.accent} stroke-width=\"1.35\" stroke-linecap=\"round\" stroke-linejoin=\"round\" />\n    </svg>\n  );\n}\n\nexport function renderSharedAgentGlyph(\n  kind: SharedAgentGlyphKind,\n  palette: SharedGlyphPalette,\n): JSX.Element {\n  switch (kind) {\n    case 'claude':\n      return <ClaudeGlyph palette={palette} />;\n    case 'gemini':\n      return <GeminiGlyph palette={palette} />;\n    case 'codex':\n      return <CodexGlyph palette={palette} />;\n    case 'opencode':\n      return <OpenCodeGlyph palette={palette} />;\n    case 'hydra':\n      return <HydraGlyph palette={palette} />;\n    case 'generic':\n      return <GenericGlyph palette={palette} />;\n  }\n}\n`;
    await writeFile(sharedRendererPath, sharedRendererSource, 'utf8');

    const agentGlyphPath = path.join(clone.workRoot, 'src/components/AgentGlyph.tsx');
    const agentGlyphSource = readFile(agentGlyphPath);
    const agentGlyphImported = replaceOnce(
      agentGlyphSource,
      "import { theme } from '../lib/theme';\n",
      "import { theme } from '../lib/theme';\nimport { renderSharedAgentGlyph } from '../lib/agent-glyph-renderer';\n",
      'AgentGlyph shared renderer import',
    );
    const agentGlyphMutated = replaceSpan(
      agentGlyphImported,
      'function ClaudeGlyph(props: { palette: GlyphPalette }): JSX.Element {\n',
      'export function AgentGlyph(props: AgentGlyphProps): JSX.Element {\n',
      "function renderGlyph(kind: AgentGlyphKind, palette: GlyphPalette): JSX.Element {\n  return renderSharedAgentGlyph(kind, palette);\n}\n\nexport function AgentGlyph(props: AgentGlyphProps): JSX.Element {\n",
      'AgentGlyph clone block',
    );
    await writeFile(agentGlyphPath, agentGlyphMutated, 'utf8');

    const remoteGlyphPath = path.join(clone.workRoot, 'src/remote/RemoteAgentGlyph.tsx');
    const remoteGlyphSource = readFile(remoteGlyphPath);
    const remoteGlyphImported = replaceOnce(
      remoteGlyphSource,
      "import { normalizeRemoteAgentGlyphKind, type RemoteAgentGlyphKind } from './agent-presentation';\n",
      "import { normalizeRemoteAgentGlyphKind, type RemoteAgentGlyphKind } from './agent-presentation';\nimport { renderSharedAgentGlyph } from '../lib/agent-glyph-renderer';\n",
      'RemoteAgentGlyph shared renderer import',
    );
    const remoteGlyphMutated = replaceSpan(
      remoteGlyphImported,
      'function ClaudeGlyph(props: { palette: GlyphPalette }): JSX.Element {\n',
      'export function RemoteAgentGlyph(props: RemoteAgentGlyphProps): JSX.Element {\n',
      "function renderGlyph(kind: RemoteAgentGlyphKind, palette: GlyphPalette): JSX.Element {\n  return renderSharedAgentGlyph(kind, palette);\n}\n\nexport function RemoteAgentGlyph(props: RemoteAgentGlyphProps): JSX.Element {\n",
      'RemoteAgentGlyph clone block',
    );
    await writeFile(remoteGlyphPath, remoteGlyphMutated, 'utf8');

    const afterResponses = runMcpRequests(clone.workRoot, [
      request(10, 'scan', { path: clone.workRoot }),
      request(11, 'findings', { limit: 12 }),
    ]);

    const beforeFindings = normalizedPayload(parseToolResponse(beforeResponses, 2), clone.workRoot);
    const afterFindings = normalizedPayload(parseToolResponse(afterResponses, 11), clone.workRoot);
    const files = ['src/components/AgentGlyph.tsx', 'src/remote/RemoteAgentGlyph.tsx'];
    const payload = {
      scenario: 'clone_cleanup',
      before: {
        clone: summarizeCloneFamily(beforeFindings, files),
      },
      after: {
        clone: summarizeCloneFamily(afterFindings, files),
      },
    };

    return {
      name: 'clone-cleanup',
      payload,
      markdown: buildCloneMarkdown(payload),
    };
  } finally {
    await clone.cleanup();
  }
}

async function writeScenarioArtifacts(result) {
  const scenarioDir = path.join(outputDir, result.name);
  await writeJson(path.join(scenarioDir, 'result.json'), result.payload);
  await writeMarkdown(path.join(scenarioDir, 'result.md'), result.markdown);
}

async function main() {
  assertPathExists(sentruxBin, 'built sentrux binary');
  assertPathExists(parallelCodeRoot, 'parallel-code repo');
  assertPathExists(rulesSource, 'parallel-code rules file');

  const results = [];
  results.push(await runOwnershipRegressionScenario());
  results.push(await runPropagationCleanupScenario());
  results.push(await runCloneCleanupScenario());

  for (const result of results) {
    await writeScenarioArtifacts(result);
  }

  const summary = {
    generated_at: new Date().toISOString(),
    scenarios: results.map((result) => ({
      name: result.name,
      output_dir: `./${result.name}`,
    })),
  };
  await writeJson(path.join(outputDir, 'index.json'), summary);

  const lines = [
    '# Parallel-Code Proof Runs',
    '',
    'Generated disposable-clone proof artifacts for the current `parallel-code` proof targets.',
    '',
  ];
  for (const result of results) {
    lines.push(`- \`${result.name}\`: ./${result.name}/result.md`);
  }
  lines.push('');
  await writeMarkdown(path.join(outputDir, 'README.md'), `${lines.join('\n')}\n`);

  console.log(`Wrote parallel-code proof artifacts to ${outputDir}`);
}

main().catch((error) => {
  console.error(error instanceof Error ? error.stack ?? error.message : String(error));
  process.exitCode = 1;
});
