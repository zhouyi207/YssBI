import { createPersistedWindow } from './createPersistedWindow';
import { createEphemeralWindowLabel } from './windowLabels';
import { logger } from '@/utils/appLogger';
import { normalizeIpcError } from '@/services/ipc';
import { i18n } from '@/app/i18n';

export async function openLogsWindow(): Promise<void> {
  try {
    const label = createEphemeralWindowLabel('logs');
    await createPersistedWindow({
      geometry: { source: 'backend', kind: 'logs' },
      label,
      url: 'index.html#/logs',
      title: i18n.t('log.title'),
    });
  } catch (error) {
    const ipcError = normalizeIpcError('open_logs_window', error);
    logger.app.error(
      `Failed to open logs window code=${ipcError.code} incidentId=${ipcError.incidentId ?? 'none'}`,
      'Window',
    );
    throw error;
  }
}
