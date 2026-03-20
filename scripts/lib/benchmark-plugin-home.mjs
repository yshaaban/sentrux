import { existsSync } from 'node:fs';
import { cp, mkdir } from 'node:fs/promises';
import path from 'node:path';

const TYPESCRIPT_BENCHMARK_PLUGINS = [
  'typescript',
  'javascript',
  'json',
  'css',
  'scss',
  'html',
  'yaml',
  'toml',
  'bash',
  'markdown',
];

export async function prepareTypeScriptBenchmarkHome({
  tempRoot,
  sourceHome = process.env.HOME,
}) {
  if (!tempRoot) {
    throw new Error('tempRoot is required');
  }
  if (!sourceHome) {
    throw new Error('HOME is not set');
  }

  const sourcePlugins = path.join(sourceHome, '.sentrux', 'plugins');
  const homeRoot = path.join(tempRoot, 'home');
  const targetPlugins = path.join(homeRoot, '.sentrux', 'plugins');
  await mkdir(targetPlugins, { recursive: true });

  for (const name of TYPESCRIPT_BENCHMARK_PLUGINS) {
    const sourcePlugin = path.join(sourcePlugins, name);
    if (!existsSync(sourcePlugin)) {
      continue;
    }

    await cp(sourcePlugin, path.join(targetPlugins, name), { recursive: true });
  }

  if (!existsSync(path.join(targetPlugins, 'typescript'))) {
    throw new Error(`Missing typescript plugin under ${sourcePlugins}`);
  }

  return homeRoot;
}
