import type { LogLevel } from "@/shared/types/dto/log";

/** Development and production both collect INFO and above. */
export function isFrontendLogEnabled(level: LogLevel): boolean {
  return level === "info" || level === "warn" || level === "error";
}

export const FRONTEND_LOG_BATCH_MAX_ENTRIES = 32;
export const FRONTEND_LOG_BATCH_MAX_PENDING = 256;
export const FRONTEND_LOG_BATCH_MAX_DELAY_MS = 100;
export const FRONTEND_LOG_MESSAGE_MAX_BYTES = 16_384;
