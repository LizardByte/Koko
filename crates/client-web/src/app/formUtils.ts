/** Parses a multiline path input into folder entries. */
export function parsePathsInput(value: FormDataEntryValue | null | undefined): string[] {
  return String(value ?? '')
    .split(/\r?\n/)
    .map((entry) => entry.trim())
    .filter(Boolean);
}

/** Joins path entries for display in multiline textareas. */
export function joinPaths(paths: string[]): string {
  return paths.join('\n');
}

/** Parses a comma-separated metadata language field into unique locale codes. */
export function parseMetadataLanguageInput(value: FormDataEntryValue | null): string[] {
  const languages = String(value ?? '')
    .split(',')
    .map((language) => language.trim())
    .filter(Boolean);
  return languages.length ? languages : ['en-US'];
}

/** Ensures metadata language lists always contain at least one locale. */
export function normalizedMetadataLanguages(languages?: string[]): string[] {
  const normalized = (languages ?? [])
    .map((language) => language.trim())
    .filter(Boolean);
  return normalized.length ? Array.from(new Set(normalized)) : ['en-US'];
}

/** Parses and clamps an integer form value. */
export function parseBoundedInteger(
  value: FormDataEntryValue | null,
  fallback: number,
  min: number,
  max: number,
): number {
  const parsed = Number(value ?? fallback);
  if (!Number.isFinite(parsed)) {
    return fallback;
  }

  return Math.max(min, Math.min(max, Math.floor(parsed)));
}
