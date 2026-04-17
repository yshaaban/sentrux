import { mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
export { repoRootFromImportMeta } from '../lib/script-artifacts.mjs';

export async function readJsonFile(targetPath) {
  const source = await readFile(targetPath, 'utf8');
  return JSON.parse(source);
}

export async function writeMaybe(targetPath, text) {
  if (!targetPath) {
    return;
  }

  await mkdir(path.dirname(targetPath), { recursive: true });
  await writeFile(targetPath, text, 'utf8');
}
