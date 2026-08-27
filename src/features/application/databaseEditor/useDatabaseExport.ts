import { useCallback } from 'react';
import { DatabaseService } from '@/services/database/databaseService';
import { selectDatabaseExportPath } from '@/services/platform/pathDialog';
import {
  captureProjectIdentity,
  isCurrentProjectIdentity,
} from '@/features/core/projectLifecycle/projectLifecycleAuthority';
import { logger } from '@/utils/appLogger';

export function useDatabaseExport(selectedDatabaseId: string | null): () => Promise<void> {
  return useCallback(async () => {
    if (!selectedDatabaseId) return;
    const identity = captureProjectIdentity();
    const selected = await selectDatabaseExportPath();
    if (!selected.ok || selected.value === null || !isCurrentProjectIdentity(identity)) return;
    try {
      const format = selected.value.endsWith('.parquet') ? 'parquet' : 'csv';
      await DatabaseService.exportDatabase(
        identity.projectInstanceId,
        selectedDatabaseId,
        selected.value,
        format,
      );
    } catch {
      if (isCurrentProjectIdentity(identity)) {
        logger.data.error('database export failed', 'DatabaseEditorWindow');
      }
    }
  }, [selectedDatabaseId]);
}
