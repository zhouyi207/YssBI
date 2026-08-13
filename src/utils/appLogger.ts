/**
 * Unified Application Logger
 *
 * 所有日志统一经过 Rust LogManager 处理：
 *   FE logger/console → LogService.frontendLog() → Rust LogManager
 *     → 终端(tauri_plugin_log) + 文件 + emit("log-message") → LogWindow
 *
 * 前端只负责两件事：
 *   1. 输出到浏览器 DevTools（原始 console）
 *   2. 发送到 Rust LogManager（invoke）
 *
 * LogWindow 只有一个数据源：listen("log-message")
 */

import { LogService } from '@/services/log';
import { LogLevel, LogType } from '@/shared/types/ui';

// ─── 保存原始 console 方法（拦截前） ───

const _console = {
  log:   console.log.bind(console),
  debug: console.debug.bind(console),
  info:  console.info.bind(console),
  warn:  console.warn.bind(console),
  error: console.error.bind(console),
};

// ─── 工具函数 ───

function formatArgs(args: unknown[]): string {
  return args.map(arg => {
    if (typeof arg === 'string') return arg;
    try { return JSON.stringify(arg, null, 2); } catch { return String(arg); }
  }).join(' ');
}

function sendToRust(level: LogLevel, logType: LogType, message: string, source?: string) {
  LogService.frontendLog(level, logType, message, source).catch(() => {});
}

// ─── Console 拦截：所有 console 调用 → DevTools + Rust LogManager ───

const CONSOLE_MAP: Array<{
  fn: 'log' | 'debug' | 'info' | 'warn' | 'error';
  level: LogLevel;
}> = [
  { fn: 'log',   level: LogLevel.Info  },
  { fn: 'debug', level: LogLevel.Debug },
  { fn: 'info',  level: LogLevel.Info  },
  { fn: 'warn',  level: LogLevel.Warn  },
  { fn: 'error', level: LogLevel.Error },
];

for (const { fn, level } of CONSOLE_MAP) {
  const original = _console[fn];
  console[fn] = (...args: unknown[]) => {
    original(...args);
    const message = formatArgs(args);
    sendToRust(level, LogType.System, message);
  };
}

// ─── 分类 Logger API ───

const TYPE_LABELS: Record<LogType, string> = {
  [LogType.Application]: 'APP',
  [LogType.Execution]:   'EXEC',
  [LogType.System]:      'SYS',
  [LogType.Graph]:       'GRAPH',
  [LogType.Data]:        'DATA',
  [LogType.Notify]:      'NOTIFY',
};

const CONSOLE_FN: Record<LogLevel, (...args: unknown[]) => void> = {
  [LogLevel.Trace]: _console.debug,
  [LogLevel.Debug]: _console.debug,
  [LogLevel.Info]:  _console.log,
  [LogLevel.Warn]:  _console.warn,
  [LogLevel.Error]: _console.error,
};

function emit(level: LogLevel, logType: LogType, message: string, source?: string) {
  const tag = TYPE_LABELS[logType] ?? logType.toUpperCase();
  const prefix = source ? `[${tag}][${source}]` : `[${tag}]`;
  const formatted = `${prefix} ${message}`;

  CONSOLE_FN[level](formatted);
  sendToRust(level, logType, message, source);
}

function createTypedLogger(logType: LogType) {
  return {
    trace: (msg: string, source?: string) => emit(LogLevel.Trace, logType, msg, source),
    debug: (msg: string, source?: string) => emit(LogLevel.Debug, logType, msg, source),
    info:  (msg: string, source?: string) => emit(LogLevel.Info,  logType, msg, source),
    warn:  (msg: string, source?: string) => emit(LogLevel.Warn,  logType, msg, source),
    error: (msg: string, source?: string) => emit(LogLevel.Error, logType, msg, source),
  };
}

export const logger = {
  app:   createTypedLogger(LogType.Application),
  exec:  createTypedLogger(LogType.Execution),
  sys:   createTypedLogger(LogType.System),
  graph: createTypedLogger(LogType.Graph),
  data:   createTypedLogger(LogType.Data),
  notify: createTypedLogger(LogType.Notify),
};
