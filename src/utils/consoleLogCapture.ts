import type { LogLevel } from "@/shared/types/dto/log";

let suppressed = 0;

export function withoutConsoleCapture(action: () => void): void {
  suppressed += 1;
  try {
    action();
  } finally {
    suppressed -= 1;
  }
}

const METHODS = {
  log: "info",
  info: "info",
  debug: "debug",
  trace: "trace",
  warn: "warn",
  error: "error",
} as const satisfies Record<string, LogLevel>;

export function installConsoleLogCapture(
  onLog: (level: LogLevel, args: readonly unknown[]) => void,
): () => void {
  let active = true;
  const restore: Array<() => void> = [];
  for (const [method, level] of Object.entries(METHODS)) {
    const key = method as keyof typeof METHODS;
    const original = console[key];
    const wrapped = (...args: unknown[]) => {
      original.apply(console, args);
      if (!active || suppressed > 0) return;
      // Serialization or forwarding failures must never produce another console record.
      withoutConsoleCapture(() => {
        try {
          onLog(level, args);
        } catch {
          /* Logging cannot change the caller's result. */
        }
      });
    };
    console[key] = wrapped;
    restore.push(() => {
      if (console[key] === wrapped) console[key] = original;
    });
  }
  return () => {
    active = false;
    restore.forEach((restoreMethod) => restoreMethod());
  };
}

/** Objects are summarized; logging must not serialize arbitrary user datasets or documents. */
export function consoleMessage(args: readonly unknown[], maxCharacters: number): string {
  let remaining = maxCharacters;
  const parts: string[] = [];
  for (const arg of args.slice(0, 32)) {
    if (remaining <= 0) break;
    let value: string;
    if (arg instanceof Error) value = `${arg.name}: ${arg.message}`;
    else if (Array.isArray(arg)) value = `[Array(${arg.length})]`;
    else if (arg !== null && typeof arg === "object") value = "[Object]";
    else if (typeof arg === "function") value = "[Function]";
    else value = String(arg);
    parts.push(value.slice(0, remaining));
    remaining -= Math.min(value.length, remaining) + 1;
  }
  return parts.join(" ") || "(empty console message)";
}
