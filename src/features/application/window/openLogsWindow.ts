import { createPersistedWindow } from './createPersistedWindow';
import { createEphemeralWindowLabel } from './windowLabels';
import { uiStore } from '@/features/core/ui/UIStore';
import { logger } from '@/utils/appLogger';
import { i18n } from '@/app/i18n';

export type OpenLogsWindowOptions = {
  fallbackX?: number;
  fallbackY?: number;
};

export async function openLogsWindow(options?: OpenLogsWindowOptions): Promise<void> {
  try {
    const label = createEphemeralWindowLabel('logs');
    await createPersistedWindow({
      kind: 'logs',
      label,
      url: 'index.html#/logs',
      title: i18n.t('log.title'),
      fallbackX: typeof options?.fallbackX === 'number' ? options.fallbackX : undefined,
      fallbackY: typeof options?.fallbackY === 'number' ? options.fallbackY : undefined,
    });
  } catch (error) {
    logger.app.error(
      `Failed to open logs window: ${error instanceof Error ? error.message : String(error)}`,
      'Window',
    );
    uiStore.showToast(i18n.t('log.failedOpenWindow'), 'error');
  }
}
