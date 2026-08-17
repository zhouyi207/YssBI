import { i18n } from '@/app/i18n';
import { logger } from '@/utils/appLogger';
import { normalizeIpcError } from '@/services/ipc';
import { createPersistedWindow } from './createPersistedWindow';
import { createEphemeralWindowLabel } from './windowLabels';

export async function openBayesWindow(): Promise<void> {
  try {
    const label = createEphemeralWindowLabel('bayes');
    await createPersistedWindow({
      geometry: { source: 'backend', kind: 'bayes' },
      label,
      url: 'index.html#/bayes',
      title: i18n.t('bayes.title'),
    });
  } catch (error) {
    const ipcError = normalizeIpcError('open_bayes_window', error);
    logger.app.error(
      `Failed to open Bayes window code=${ipcError.code} incidentId=${ipcError.incidentId ?? 'none'}`,
      'Window',
    );
    throw error;
  }
}
