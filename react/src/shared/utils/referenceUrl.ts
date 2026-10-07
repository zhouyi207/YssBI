/** Only remote web documents belong in a reference panel. */
export function parseReferenceUrl(value: unknown): URL | null {
  if (typeof value !== "string") return null;
  try {
    const url = new URL(value);
    return (url.protocol === "https:" || url.protocol === "http:") && !url.username && !url.password
      ? url
      : null;
  } catch {
    return null;
  }
}
