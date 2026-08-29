import { LogService } from '@/services/log';
import {
  createFrontendDiagnosticBatcher,
  type FrontendDiagnosticEntry,
} from '@/utils/frontendDiagnosticBatcher';
import {
  FRONTEND_DIAGNOSTIC_BATCH_MAX_DELAY_MS,
  FRONTEND_DIAGNOSTIC_BATCH_MAX_ENTRIES,
  FRONTEND_DIAGNOSTIC_BATCH_MAX_PENDING,
  FRONTEND_DIAGNOSTIC_MESSAGE_MAX_BYTES,
} from '@/utils/logConfig';

type DiagnosticLevel = FrontendDiagnosticEntry['level'];
type DiagnosticDomain = FrontendDiagnosticEntry['domain'];

const CONSOLE_METHOD: Record<DiagnosticLevel, 'debug' | 'log' | 'warn' | 'error'> = {
  trace: 'debug',
  debug: 'debug',
  info: 'log',
  warn: 'warn',
  error: 'error',
};

const batcher = createFrontendDiagnosticBatcher({
  maxBatchEntries: FRONTEND_DIAGNOSTIC_BATCH_MAX_ENTRIES,
  maxPendingEntries: FRONTEND_DIAGNOSTIC_BATCH_MAX_PENDING,
  maxDelayMs: FRONTEND_DIAGNOSTIC_BATCH_MAX_DELAY_MS,
  maxMessageBytes: FRONTEND_DIAGNOSTIC_MESSAGE_MAX_BYTES,
  submit: (entries) => LogService.submitFrontendDiagnostics(entries),
});

function createEntry(
  level: DiagnosticLevel,
  domain: DiagnosticDomain,
  message: string,
  source?: string,
): FrontendDiagnosticEntry {
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
  level: DiagnosticLevel,
  domain: DiagnosticDomain,
  label: string,
  message: string,
  source?: string,
): void {
  const normalizedSource = source?.trim();
  const prefix = normalizedSource ? `[${label}][${normalizedSource}]` : `[${label}]`;
  console[CONSOLE_METHOD[level]](`${prefix} ${message}`);
  batcher.enqueue(createEntry(level, domain, message, source));
}

function createTypedLogger(domain: DiagnosticDomain, label: string) {
  return {
    trace: (message: string, source?: string) => emit('trace', domain, label, message, source),
    debug: (message: string, source?: string) => emit('debug', domain, label, message, source),
    info: (message: string, source?: string) => emit('info', domain, label, message, source),
    warn: (message: string, source?: string) => emit('warn', domain, label, message, source),
    error: (message: string, source?: string) => emit('error', domain, label, message, source),
  };
}

export const logger = {
  app: createTypedLogger('application', 'APP'),
  exec: createTypedLogger('execution', 'EXEC'),
  sys: createTypedLogger('system', 'SYS'),
  graph: createTypedLogger('graph', 'GRAPH'),
  data: createTypedLogger('data', 'DATA'),
};
