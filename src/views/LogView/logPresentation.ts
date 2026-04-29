import type { LogLevel, LogType } from "@/shared/types/ui";

export const LOG_TYPE_LABELS: Record<string, string> = {
  application: "APP",
  execution: "EXEC",
  system: "SYS",
  graph: "GRAPH",
  data: "DATA",
};

export const LOG_TYPE_BACKGROUND: Record<string, string> = {
  application: "bg-green-500/10",
  execution: "bg-purple-500/10",
  system: "bg-cyan-500/10",
  graph: "bg-orange-500/10",
  data: "bg-pink-500/10",
};

export function getLogLevelColor(level: LogLevel) {
  switch (level) {
    case "error": return "text-red-400";
    case "warn": return "text-yellow-400";
    case "info": return "text-blue-400";
    case "debug": return "text-gray-400";
    case "trace": return "text-gray-500";
    default: return "text-gray-400";
  }
}

export function getLogLevelBackground(level: LogLevel) {
  switch (level) {
    case "error": return "bg-red-500/10";
    case "warn": return "bg-yellow-500/10";
    case "info": return "bg-blue-500/10";
    case "debug": return "bg-gray-500/10";
    case "trace": return "bg-gray-600/10";
    default: return "bg-gray-500/10";
  }
}

export function getLogTypeColor(type: LogType) {
  switch (type) {
    case "application": return "text-green-400";
    case "execution": return "text-purple-400";
    case "system": return "text-cyan-400";
    case "graph": return "text-orange-400";
    case "data": return "text-pink-400";
    default: return "text-gray-400";
  }
}
