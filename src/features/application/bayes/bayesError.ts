import type { TaskErrorDetailsDTO } from '@/shared/types/bayes';
import { IpcError } from '@/services/ipc';

const DETAIL_KEYS = ['column', 'row', 'parameter', 'path'] as const;

type DetailKey = typeof DETAIL_KEYS[number];

export interface BayesApplicationError {
  code: string;
  details: TaskErrorDetailsDTO | null;
  incidentId: string | null;
}

export function normalizeBayesApplicationError(
  caught: unknown,
  fallbackCode: string,
): BayesApplicationError {
  if (!(caught instanceof IpcError)) {
    return { code: fallbackCode, details: null, incidentId: null };
  }
  return {
    code: caught.code,
    details: normalizeBayesErrorDetails(caught.details),
    incidentId: caught.incidentId,
  };
}

function normalizeBayesErrorDetails(value: unknown): TaskErrorDetailsDTO | null {
  if (value === null) return null;
  if (!isRecord(value)
    || Object.keys(value).some(key => !DETAIL_KEYS.includes(key as DetailKey))) {
    return null;
  }

  const details: TaskErrorDetailsDTO = {};
  if (value.column !== undefined && value.column !== null) {
    if (!isNonEmptyString(value.column)) return null;
    details.column = value.column;
  }
  if (value.row !== undefined && value.row !== null) {
    if (!Number.isSafeInteger(value.row) || (value.row as number) < 0) return null;
    details.row = value.row as number;
  }
  if (value.parameter !== undefined && value.parameter !== null) {
    if (!isNonEmptyString(value.parameter)) return null;
    details.parameter = value.parameter;
  }
  if (value.path !== undefined && value.path !== null) {
    if (!isNonEmptyString(value.path)) return null;
    details.path = value.path;
  }
  return Object.keys(details).length === 0 ? null : details;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function isNonEmptyString(value: unknown): value is string {
  return typeof value === 'string' && value.trim().length > 0;
}
