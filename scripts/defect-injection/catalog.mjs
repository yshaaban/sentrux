import { createDogfoodCatalog as createDogfoodRepoCatalog } from './catalog-dogfood.mjs';
import { createParallelCodeCatalog as createParallelCodeRepoCatalog } from './catalog-parallel-code.mjs';

export function createParallelCodeCatalog() {
  return createParallelCodeRepoCatalog();
}

export function createDogfoodCatalog() {
  return createDogfoodRepoCatalog();
}

export function selectDefects(catalog, defectIds) {
  if (!Array.isArray(defectIds) || defectIds.length === 0) {
    return catalog;
  }

  const selected = new Set(defectIds);
  return catalog.filter((defect) => selected.has(defect.id));
}
