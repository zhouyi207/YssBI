import type { LogDomainId } from "@/features/application/log";
import type { DiagnosticLevel } from "@/shared/types/domain/diagnostics";

export const LOG_DOMAIN_TITLE_KEYS = {
  all: "log.domains.all",
  application: "log.domains.application",
  execution: "log.domains.execution",
  system: "log.domains.system",
  graph: "log.domains.graph",
  data: "log.domains.data",
  ui: "log.domains.ui",
} as const satisfies Record<LogDomainId, string>;

export const LOG_DOMAIN_LABELS: Record<string, string> = {
  application: "APP",
  execution: "EXEC",
  system: "SYS",
  graph: "GRAPH",
  data: "DATA",
  ui: "UI",
};

export const LOG_DOMAIN_BACKGROUND: Record<string, string> = {
  application: "bg-green-500/10",
  execution: "bg-purple-500/10",
  system: "bg-cyan-500/10",
  graph: "bg-orange-500/10",
  data: "bg-pink-500/10",
  ui: "bg-amber-500/10",
};

export function getLogLevelColor(level: DiagnosticLevel) {
  switch (level) {
    case "error":
      return "text-red-400";
    case "warn":
      return "text-yellow-400";
    case "info":
      return "text-blue-400";
    case "debug":
      return "text-muted-foreground";
    case "trace":
      return "text-muted-foreground/80";
  }
}

export function getLogLevelBackground(level: DiagnosticLevel) {
  switch (level) {
    case "error":
      return "bg-red-500/10";
    case "warn":
      return "bg-yellow-500/10";
    case "info":
      return "bg-blue-500/10";
    case "debug":
      return "bg-muted/60";
    case "trace":
      return "bg-muted/40";
  }
}

export function formatDiagnosticTime(timestamp: string): string {
  return timestamp.match(/(?:T|\s)(\d{2}:\d{2}:\d{2})/)?.[1] ?? timestamp;
}

export function getLogDomainColor(domain: string) {
  switch (domain) {
    case "application":
      return "text-green-400";
    case "execution":
      return "text-purple-400";
    case "system":
      return "text-cyan-400";
    case "graph":
      return "text-orange-400";
    case "data":
      return "text-pink-400";
    case "ui":
      return "text-amber-400";
    default:
      return "text-muted-foreground";
  }
}
