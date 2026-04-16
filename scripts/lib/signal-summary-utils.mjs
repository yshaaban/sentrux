export function asArray(value) {
  return Array.isArray(value) ? value : [];
}

export function safeRatio(numerator, denominator) {
  if (!Number.isFinite(numerator) || !Number.isFinite(denominator) || denominator <= 0) {
    return null;
  }

  return Number((numerator / denominator).toFixed(3));
}

export function ensureMapEntry(map, key, createEntry) {
  if (!map.has(key)) {
    map.set(key, createEntry(key));
  }

  return map.get(key);
}
