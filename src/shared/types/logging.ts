export type LogLevel = 'trace' | 'debug' | 'info' | 'warn' | 'error';
export type LogType = 'application' | 'execution' | 'system';

export interface LogMessage {
  timestamp: string;
  level: LogLevel;
  log_type: LogType;
  message: string;
  source?: string;
}

export interface LogFilter {
  levels: Set<LogLevel>;
  types: Set<LogType>;
  searchText: string;
}
