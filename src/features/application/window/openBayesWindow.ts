import { i18n } from '@/app/i18n';
import { uiStore } from '@/features/core/ui/UIStore';
import { logger } from '@/utils/appLogger';
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
    logger.app.error(
      `Failed to open Bayes window: ${error instanceof Error ? error.message : String(error)}`,
      'Window',
    );
    uiStore.showToast(i18n.t('bayes.failedOpenWindow'), 'error');
  }
}
