import { createPersistedWindow } from './createPersistedWindow';
import { createEphemeralWindowLabel } from './windowLabels';
import { uiStore } from '@/features/core/ui/UIStore';
import { logger } from '@/utils/appLogger';
import { i18n } from '@/app/i18n';

export async function openDatabaseEditorWindow(databaseId?: string): Promise<void> {
  try {
    const label = createEphemeralWindowLabel('dataview');
    const url = databaseId
      ? `index.html?database=${encodeURIComponent(databaseId)}#/database`
      : 'index.html#/database';
    await createPersistedWindow({
      geometry: { source: 'backend', kind: 'databaseEditor' },
      label,
      url,
      title: i18n.t('databaseEditor.title'),
    });
  } catch (error) {
    logger.app.error(
      `Failed to open data view: ${error instanceof Error ? error.message : String(error)}`,
      'Window',
    );
    uiStore.showToast(i18n.t('databaseEditor.failedOpenWindow'), 'error');
  }
}
