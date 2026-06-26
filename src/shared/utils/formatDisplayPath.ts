/**
 * Normalize filesystem paths for UI display across platforms.
 * Strips Windows extended-length prefixes (`\\?\`, `\\?\UNC\`).
 */
export function formatDisplayPath(path: string): string {
  if (!path) return path;

  if (path.startsWith("\\\\?\\UNC\\")) {
    return `\\\\${path.slice("\\\\?\\UNC\\".length)}`;
  }
  if (path.startsWith("\\\\?\\")) {
    return path.slice(4);
  }

  return path;
}

/** Case-insensitive path comparison after display normalization. */
export function pathsEqualForCompare(a: string, b: string): boolean {
  return (
    formatDisplayPath(a).replace(/\\/g, "/").toLowerCase() ===
    formatDisplayPath(b).replace(/\\/g, "/").toLowerCase()
  );
}
