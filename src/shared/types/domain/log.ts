export const LOG_LEVELS = ["trace", "debug", "info", "warn", "error"] as const;

export type LogLevel = (typeof LOG_LEVELS)[number];

export const LOG_ORIGINS = ["rust", "frontend"] as const;
export type LogOrigin = (typeof LOG_ORIGINS)[number];

export const LOG_DOMAINS = ["application", "execution", "system", "graph", "data", "ui"] as const;
export type LogDomain = (typeof LOG_DOMAINS)[number];

export type LogFieldValueDto =
  | null
  | boolean
  | number
  | string
  | LogFieldValueDto[]
  | { [key: string]: LogFieldValueDto };

export type LogFieldsDto = Record<string, LogFieldValueDto>;

export interface LogRecordDto {
  streamId: string;
  sequence: number;
  timestamp: string;
  level: LogLevel;
  origin: LogOrigin;
  domain: LogDomain;
  target: string;
  event?: string;
  message: string;
  source?: string;
  fields: LogFieldsDto;
}

export interface LogSubscriptionDto {
  subscriptionId: string;
  streamId: string;
  entries: LogRecordDto[];
  latestSequence: number;
  truncated: boolean;
}

export interface LogBatchDto {
  streamId: string;
  entries: LogRecordDto[];
  failure?: "storage_unavailable";
}

export interface LogQuery {
  beforeSequence?: number;
  level?: LogLevel;
  origin?: LogOrigin;
  limit?: number;
}

export interface LogPage {
  entries: LogRecordDto[];
  nextBeforeSequence: number | null;
}

export interface LogStatistics {
  total: number;
  latestSequence: number;
  byLevel: Partial<Record<LogLevel, number>>;
  byOrigin: Partial<Record<LogOrigin, number>>;
}

/** Payload accepted by `plugin:tracing|submit_frontend_logs`; Rust assigns stream metadata. */
export interface FrontendLogEntryDto {
  level: LogLevel;
  domain: LogDomain;
  target: string;
  event?: string;
  message: string;
  source?: string;
  fields: LogFieldsDto;
}
