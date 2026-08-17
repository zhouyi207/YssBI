export const DIAGNOSTIC_LEVELS = [
  'trace',
  'debug',
  'info',
  'warn',
  'error',
] as const;

export type DiagnosticLevel = (typeof DIAGNOSTIC_LEVELS)[number];

export const DIAGNOSTIC_ORIGINS = ['rust', 'frontend'] as const;
export type DiagnosticOrigin = (typeof DIAGNOSTIC_ORIGINS)[number];

export const DIAGNOSTIC_DOMAINS = [
  'application',
  'execution',
  'system',
  'graph',
  'data',
  'ui',
] as const;
export type DiagnosticDomain = (typeof DIAGNOSTIC_DOMAINS)[number];

export type DiagnosticFieldValueDto =
  | null
  | boolean
  | number
  | string
  | DiagnosticFieldValueDto[]
  | { [key: string]: DiagnosticFieldValueDto };

export type DiagnosticFieldsDto = Record<string, DiagnosticFieldValueDto>;

export interface DiagnosticRecordDto {
  streamId: string;
  sequence: number;
  timestamp: string;
  level: DiagnosticLevel;
  origin: DiagnosticOrigin;
  domain: DiagnosticDomain;
  target: string;
  event?: string;
  message: string;
  source?: string;
  fields: DiagnosticFieldsDto;
}

export interface DiagnosticSubscriptionDto {
  subscriptionId: string;
  streamId: string;
  entries: DiagnosticRecordDto[];
  latestSequence: number;
  truncated: boolean;
}

export interface DiagnosticBatchDto {
  streamId: string;
  entries: DiagnosticRecordDto[];
}

/** Payload accepted by `submit_frontend_diagnostics`; Rust assigns stream metadata. */
export interface FrontendDiagnosticEntryDto {
  level: DiagnosticLevel;
  domain: DiagnosticDomain;
  target: string;
  event?: string;
  message: string;
  source?: string;
  fields: DiagnosticFieldsDto;
}
