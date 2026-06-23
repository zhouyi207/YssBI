/**
 * Normalize unknown errors (Error, string, Tauri FrontendError `{ code, message }`, etc.)
 * into a user-facing message string.
 */
export function formatErrorMessage(error: unknown, fallback = "Unknown error"): string {
  if (error == null) return fallback;
  if (typeof error === "string") {
    const trimmed = error.trim();
    return trimmed.length > 0 ? trimmed : fallback;
  }
  if (error instanceof Error) {
    const trimmed = error.message.trim();
    return trimmed.length > 0 ? trimmed : fallback;
  }
  if (typeof error === "object") {
    const record = error as Record<string, unknown>;
    const message = record.message;
    if (typeof message === "string" && message.trim().length > 0) {
      return message.trim();
    }
    const code = record.code;
    if (typeof code === "string" && code.trim().length > 0) {
      return code.trim();
    }
  }
  const asString = String(error);
  return asString === "[object Object]" ? fallback : asString;
}
