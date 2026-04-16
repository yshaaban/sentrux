import { existsSync } from 'node:fs';
import { readFile } from 'node:fs/promises';
import path from 'node:path';

function loadJson(targetPath) {
  return readFile(targetPath, 'utf8').then((source) => JSON.parse(source));
}

function sourceLabelFromPath(targetPath) {
  const baseName = path.basename(targetPath);
  return baseName.endsWith('.json') ? baseName.slice(0, -5) : baseName;
}

async function loadBatchArtifact(batchPath, kind) {
  const batch = await loadJson(batchPath);
  const batchDir = path.dirname(path.resolve(batchPath));
  const bundleFileName = kind === 'codex-batch' ? 'codex-session.json' : 'diff-replay.json';
  const bundlePaths = [];
  const entries = [];

  for (const result of batch.results ?? []) {
    const outputDir = result.output_dir ? path.resolve(batchDir, result.output_dir) : null;
    if (!outputDir) {
      throw new Error(`Missing output_dir for batch result in ${batchPath}`);
    }

    const bundlePath = path.join(outputDir, bundleFileName);
    if (!existsSync(bundlePath)) {
      throw new Error(`Missing bundle artifact: ${bundlePath}`);
    }

    bundlePaths.push(bundlePath);
    const bundle = await loadJson(bundlePath);
    entries.push({
      bundle,
      bundle_path: bundlePath,
      output_dir: outputDir,
      source_kind: kind,
      source_label:
        result.task_label ??
        result.task_id ??
        result.replay_id ??
        result.commit ??
        bundle.task_label ??
        bundle.task_id ??
        bundle.replay_id ??
        bundle.replay?.commit ??
        sourceLabelFromPath(bundlePath),
      task_id: result.task_id ?? bundle.task_id ?? null,
      task_label: result.task_label ?? bundle.task_label ?? null,
      replay_id: result.replay_id ?? bundle.replay_id ?? null,
      commit: result.commit ?? bundle.replay?.commit ?? null,
      expected_signal_kinds: result.expected_signal_kinds ?? bundle.expected_signal_kinds ?? [],
      expected_fix_surface: result.expected_fix_surface ?? bundle.expected_fix_surface ?? null,
    });
  }

  return {
    source_mode: kind,
    source_paths: [path.resolve(batchPath), ...bundlePaths],
    repo_root: batch.repo_root ?? entries[0]?.bundle?.repo_root ?? null,
    label: sourceLabelFromPath(batchPath),
    entries,
  };
}

export async function loadArtifactInput(args) {
  const sources = [];

  if (args.bundlePath) {
    const bundle = await loadJson(args.bundlePath);
    sources.push({
      source_mode: 'bundle',
      source_paths: [path.resolve(args.bundlePath)],
      repo_root: bundle.repo_root ?? bundle.source_root ?? null,
      label: sourceLabelFromPath(args.bundlePath),
      entries: [
        {
          bundle,
          bundle_path: path.resolve(args.bundlePath),
          output_dir: path.dirname(path.resolve(args.bundlePath)),
          source_kind: 'bundle',
          source_label:
            bundle.task_label ??
            bundle.task_id ??
            bundle.replay_id ??
            bundle.replay?.commit ??
            sourceLabelFromPath(args.bundlePath),
          task_id: bundle.task_id ?? null,
          task_label: bundle.task_label ?? null,
          replay_id: bundle.replay_id ?? null,
          commit: bundle.replay?.commit ?? null,
          expected_signal_kinds: bundle.expected_signal_kinds ?? [],
          expected_fix_surface: bundle.expected_fix_surface ?? null,
        },
      ],
    });
  }
  if (args.codexBatchPath) {
    sources.push(await loadBatchArtifact(args.codexBatchPath, 'codex-batch'));
  }
  if (args.replayBatchPath) {
    sources.push(await loadBatchArtifact(args.replayBatchPath, 'replay-batch'));
  }

  if (sources.length === 0) {
    return null;
  }
  if (sources.length === 1) {
    return sources[0];
  }

  return {
    source_mode: 'combined',
    source_paths: [...new Set(sources.flatMap((source) => source.source_paths))],
    repo_root: sources[0]?.repo_root ?? null,
    label: sources.map((source) => source.label).join('-'),
    entries: sources.flatMap((source) => source.entries),
  };
}
