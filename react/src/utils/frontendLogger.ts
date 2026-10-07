import type { FrontendLogEntryDto } from "@/shared/types/domain/log";
import { withoutConsoleCapture } from "./consoleLogCapture";
import { isFrontendLogEnabled } from "./logConfig";

type LogLevel = FrontendLogEntryDto["level"];
type LogDomain = FrontendLogEntryDto["domain"];

let logSink: ((entry: FrontendLogEntryDto) => void) | undefined;

/** Application installs the transport; an obsolete cleanup cannot detach its replacement. */
export function bindFrontendLogSink(sink: (entry: FrontendLogEntryDto) => void): () => void {
  logSink = sink;
  return () => {
    if (logSink === sink) logSink = undefined;
  };
}

const CONSOLE_METHOD: Record<LogLevel, "debug" | "log" | "warn" | "error"> = {
  trace: "debug",
  debug: "debug",
  info: "log",
  warn: "warn",
  error: "error",
};

function createEntry(
  level: LogLevel,
  domain: LogDomain,
  message: string,
  source?: string,
): FrontendLogEntryDto {
  const normalizedSource = source?.trim();
  return {
    level,
    domain,
    target: normalizedSource || `frontend.${domain}`,
    message,
    ...(normalizedSource ? { source: normalizedSource } : {}),
    fields: {},
  };
}

function emit(
  level: LogLevel,
  domain: LogDomain,
  label: string,
  message: string,
  source?: string,
): void {
  if (!isFrontendLogEnabled(level)) return;
  const entry = createEntry(level, domain, message, source);
  const prefix = entry.source ? `[${label}][${entry.source}]` : `[${label}]`;
  withoutConsoleCapture(() => console[CONSOLE_METHOD[level]](`${prefix} ${message}`));
  try {
    logSink?.(entry);
  } catch {
    // A transport failure must not change the caller's result or produce another log.
  }
}

function createTypedLogger(domain: LogDomain, label: string) {
  return {
    trace: (message: string, source?: string) => emit("trace", domain, label, message, source),
    debug: (message: string, source?: string) => emit("debug", domain, label, message, source),
    info: (message: string, source?: string) => emit("info", domain, label, message, source),
    warn: (message: string, source?: string) => emit("warn", domain, label, message, source),
    error: (message: string, source?: string) => emit("error", domain, label, message, source),
  };
}

export const logger = {
  app: createTypedLogger("application", "APP"),
  exec: createTypedLogger("execution", "EXEC"),
  sys: createTypedLogger("system", "SYS"),
  graph: createTypedLogger("graph", "GRAPH"),
  data: createTypedLogger("data", "DATA"),
};
