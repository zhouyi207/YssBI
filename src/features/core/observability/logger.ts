type DiagnosticLevel = "trace" | "debug" | "info" | "warn" | "error";
type DiagnosticDomain = "application" | "execution" | "system" | "graph" | "data" | "ui";

const CONSOLE_METHOD: Record<DiagnosticLevel, "debug" | "log" | "warn" | "error"> = {
  trace: "debug",
  debug: "debug",
  info: "log",
  warn: "warn",
  error: "error",
};

function createTypedLogger(domain: DiagnosticDomain, label: string) {
  const emit = (level: DiagnosticLevel, message: string, source?: string) => {
    const normalizedSource = source?.trim();
    const prefix = normalizedSource ? `[${label}][${normalizedSource}]` : `[${label}]`;
    console[CONSOLE_METHOD[level]](`${prefix} ${message}`);
  };
  return {
    trace: (message: string, source?: string) => emit("trace", message, source),
    debug: (message: string, source?: string) => emit("debug", message, source),
    info: (message: string, source?: string) => emit("info", message, source),
    warn: (message: string, source?: string) => emit("warn", message, source),
    error: (message: string, source?: string) => emit("error", message, source),
    domain,
  };
}

export const logger = {
  app: createTypedLogger("application", "APP"),
  exec: createTypedLogger("execution", "EXEC"),
  sys: createTypedLogger("system", "SYS"),
  graph: createTypedLogger("graph", "GRAPH"),
  data: createTypedLogger("data", "DATA"),
};
