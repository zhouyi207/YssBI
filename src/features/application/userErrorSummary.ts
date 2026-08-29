import type { TFunction } from 'i18next';
import { isApplicationIpcError } from '@/features/application/errorReference';

export interface UserErrorSummary {
  message: string;
  incidentId: string | null;
}

export function summarizeUserError(error: unknown, t: TFunction): UserErrorSummary {
  if (isApplicationIpcError(error)) {
    return {
      message: `${t('common.error')} [${error.code}]`,
      incidentId: error.incidentId,
    };
  }
  return {
    message: t('common.unexpectedError'),
    incidentId: null,
  };
}

export function formatInlineUserError(error: unknown, t: TFunction): string {
  const summary = summarizeUserError(error, t);
  return summary.incidentId
    ? `${summary.message} · ${t('common.incidentId')}: ${summary.incidentId}`
    : summary.message;
}
