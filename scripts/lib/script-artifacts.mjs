import { readFileSync } from 'node:fs';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

export function repoRootFromImportMeta(importMetaUrl, levelsUp = 2) {
  const filename = fileURLToPath(importMetaUrl);
  const dirname = path.dirname(filename);
  return path.resolve(dirname, ...Array(levelsUp).fill('..'));
}

export function readJsonSync(targetPath) {
  return JSON.parse(readFileSync(targetPath, 'utf8'));
}

export async function writeJson(targetPath, value) {
  await mkdir(path.dirname(targetPath), { recursive: true });
  await writeFile(targetPath, `${JSON.stringify(value, null, 2)}\n`, 'utf8');
}
