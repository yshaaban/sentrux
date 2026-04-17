export function parseJsonLine(value) {
  if (typeof value !== 'string') {
    return null;
  }

  const trimmed = value.trim();
  if (!trimmed) {
    return null;
  }

  try {
    return JSON.parse(trimmed);
  } catch {
    return null;
  }
}

export function parseJsonLines(value) {
  if (typeof value !== 'string') {
    return [];
  }

  return value
    .split(/\r?\n/)
    .map(parseJsonLine)
    .filter(Boolean);
}
