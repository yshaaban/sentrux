import { createDisposableRepoClone } from './disposable-repo.mjs';
import {
  createMcpSession,
  nowMs,
  roundMs,
  runBenchmarkTool,
  runRepeatedBenchmarkSamples,
  runTool,
} from './benchmark-harness.mjs';
import { prepareTypeScriptBenchmarkHome } from './benchmark-plugin-home.mjs';
import { resolveHeadCommitEpoch } from './repo-identity.mjs';

export function sanitizePublicArtifactValue(value, pathReplacements) {
  if (typeof value === 'string') {
    return pathReplacements.reduce(function replacePath(current, [target, replacement]) {
      return current.split(target).join(replacement);
    }, value);
  }

  if (Array.isArray(value)) {
    return value.map(function sanitizeEntry(entry) {
      return sanitizePublicArtifactValue(entry, pathReplacements);
    });
  }

  if (value && typeof value === 'object') {
    return Object.fromEntries(
      Object.entries(value).map(function sanitizePair([key, entry]) {
        return [key, sanitizePublicArtifactValue(entry, pathReplacements)];
      }),
    );
  }

  return value;
}

export function buildSessionEnv(fixedNowEpoch) {
  if (fixedNowEpoch == null) {
    return {};
  }

  return {
    SENTRUX_FIXED_NOW_EPOCH: String(fixedNowEpoch),
  };
}

export function createBenchmarkSession({
  binPath,
  repoRoot,
  homeOverride,
  skipGrammarDownload,
  requestTimeoutMs,
  fixedNowEpoch,
}) {
  return createMcpSession({
    binPath,
    repoRoot,
    homeOverride,
    skipGrammarDownload,
    requestTimeoutMs,
    extraEnv: buildSessionEnv(fixedNowEpoch),
  });
}

function resolveOperationArgs(operation, workRoot) {
  if (typeof operation.args === 'function') {
    return operation.args(workRoot);
  }

  return operation.args ?? {};
}

async function runOperationGroup(session, operations, workRoot) {
  const results = {};
  const startedAt = nowMs();

  for (const operation of operations) {
    results[operation.key] = await runBenchmarkTool(
      session,
      operation.label ?? operation.key,
      operation.tool,
      resolveOperationArgs(operation, workRoot),
      operation.summarize,
    );
  }

  return {
    elapsedMs: roundMs(nowMs() - startedAt),
    results,
  };
}

async function warmSession(session, operations, workRoot) {
  for (const operation of operations) {
    await runTool(session, operation.tool, resolveOperationArgs(operation, workRoot));
  }
}

function buildSessionOptions({
  binPath,
  repoRoot,
  homeOverride,
  skipGrammarDownload,
  requestTimeoutMs,
  fixedNowEpoch,
}) {
  return {
    binPath,
    repoRoot,
    homeOverride,
    skipGrammarDownload,
    requestTimeoutMs,
    fixedNowEpoch,
  };
}

export async function runBenchmarkSessionPhases({
  binPath,
  repoRoot,
  workRoot,
  homeOverride,
  skipGrammarDownload,
  requestTimeoutMs,
  fixedNowEpoch,
  coldOperations,
  warmOperations = [],
  warmCachedOperations = [],
  warmPatchSafetyOperations = [],
  warmPersistedOperations = [],
}) {
  const sessionOptions = buildSessionOptions({
    binPath,
    repoRoot,
    homeOverride,
    skipGrammarDownload,
    requestTimeoutMs,
    fixedNowEpoch,
  });
  const session = createBenchmarkSession(sessionOptions);
  let persistedSession = null;

  try {
    const cold = await runOperationGroup(session, coldOperations, workRoot);
    await warmSession(session, warmOperations, workRoot);
    const warmCached = await runOperationGroup(session, warmCachedOperations, workRoot);
    const warmPatchSafety = await runOperationGroup(session, warmPatchSafetyOperations, workRoot);

    persistedSession = createBenchmarkSession(sessionOptions);
    const warmPersisted = await runOperationGroup(
      persistedSession,
      warmPersistedOperations,
      workRoot,
    );
    await persistedSession.close();
    persistedSession = null;

    return {
      cold_process_total_ms: cold.elapsedMs,
      cold: cold.results,
      warm_cached_total_ms: warmCached.elapsedMs,
      warm_cached: warmCached.results,
      warm_persisted_total_ms: warmPersisted.elapsedMs,
      warm_persisted: warmPersisted.results,
      warm_patch_safety_total_ms: warmPatchSafety.elapsedMs,
      warm_patch_safety: warmPatchSafety.results,
      stdout_log: session.stdoutLog,
      stderr_log: session.stderrLog,
    };
  } finally {
    if (persistedSession) {
      await persistedSession.close();
    }
    await session.close();
  }
}

export function resolveFreshnessRepoRoot(analysisMode, frozenSourceRoot, repoRoot) {
  if (analysisMode === 'head_clone') {
    return frozenSourceRoot;
  }

  return repoRoot;
}

export async function runBenchmarkSampleInClone({
  sampleIndex,
  frozenSourceRoot,
  sampleLabel,
  rulesSource,
  runBenchmarkSession,
  freshnessMetadata,
}) {
  const clone = await createDisposableRepoClone({
    sourceRoot: frozenSourceRoot,
    label: sampleLabel,
    rulesSource,
    analysisMode: 'working_tree',
  });

  let benchmark;
  try {
    const fixedNowEpoch = resolveHeadCommitEpoch(clone.workRoot);
    const pluginHome = await prepareTypeScriptBenchmarkHome({ tempRoot: clone.tempRoot });
    benchmark = await runBenchmarkSession(clone.workRoot, pluginHome, fixedNowEpoch);
  } finally {
    await clone.cleanup();
  }

  return {
    sample_id: `sample_${sampleIndex + 1}`,
    generated_at: new Date().toISOString(),
    benchmark,
    freshnessMetadata,
  };
}

export async function collectFrozenBenchmarkSamples({
  sourceRoot,
  cloneLabel,
  rulesSource,
  analysisMode,
  repeatCount,
  buildFreshnessMetadata,
  runBenchmarkSession,
  sampleLabel,
}) {
  const frozenSource = await createDisposableRepoClone({
    sourceRoot,
    label: cloneLabel,
    rulesSource,
    analysisMode,
  });

  try {
    const freshnessMetadata = buildFreshnessMetadata(frozenSource.workRoot);
    const { samples } = await runRepeatedBenchmarkSamples({
      repeatCount,
      runSample: function runFrozenSample(sampleIndex) {
        return runBenchmarkSampleInClone({
          sampleIndex,
          frozenSourceRoot: frozenSource.workRoot,
          sampleLabel,
          rulesSource,
          runBenchmarkSession,
          freshnessMetadata,
        });
      },
    });

    return {
      freshnessMetadata,
      samples,
    };
  } finally {
    await frozenSource.cleanup();
  }
}
