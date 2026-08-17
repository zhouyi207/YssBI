import type { TFunction } from 'i18next';
import type { ErrorReference } from '@/services/ipc';

export function nodeCatalogErrorText(
  error: ErrorReference | null,
  t: TFunction,
): string {
  const genericText = t('nodeCatalog.loadError', {
    defaultValue: t('common.error'),
  });
  if (!error) return genericText;

  const codeText = `[${error.code}]`;
  return error.incidentId
    ? `${genericText} ${codeText} · ${t('common.incidentId')}: ${error.incidentId}`
    : `${genericText} ${codeText}`;
}
