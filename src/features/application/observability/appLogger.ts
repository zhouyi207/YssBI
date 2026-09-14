import type { FrontendLogEntry } from "@/utils/frontendLogBatcher";
import { withoutConsoleCapture } from "@/utils/consoleLogCapture";
import { isFrontendLogEnabled } from "@/utils/logConfig";
import { frontendLogBatcher } from "./frontendLogTransport";

type LogLevel = FrontendLogEntry["level"];
type LogDomain = FrontendLogEntry["domain"];

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
): FrontendLogEntry {
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
  const normalizedSource = source?.trim();
  const prefix = normalizedSource ? `[${label}][${normalizedSource}]` : `[${label}]`;
  withoutConsoleCapture(() => console[CONSOLE_METHOD[level]](`${prefix} ${message}`));
  frontendLogBatcher.enqueue(createEntry(level, domain, message, source));
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
