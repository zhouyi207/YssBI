import { useCallback } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import { useDatabaseStore } from '@/features/core/dataStore';
import { useEditorStore } from '@/features/core/editor';
import { uiStore } from '@/features/core/ui/UIStore';
import { DatabaseService } from '@/services/database/databaseService';
import { getUniqueName } from '@/shared/utils';

/** 触发导入数据弹窗（与菜单栏 Data > Import Data 相同逻辑） */
export function triggerImportData() {
  uiStore.showImportDialog({
    onSelect: async (type) => {
      if (type === 'csv') {
        try {
          const selected = await open({
            multiple: false,
            filters: [{ name: 'CSV File', extensions: ['csv'] }],
          });
          if (selected && !Array.isArray(selected)) {
            uiStore.showToast('正在从 CSV 导入数据...', 'info');
            const result = await DatabaseService.loadDatabase({
              csv: {
                path: selected,
                delimiter: ',',
                hasHeader: true,
                inferSchemaLength: 1000,
              },
            });
            const existingDbs = Object.values(useDatabaseStore.getState().databases) as Array<{ name: string }>;
            const uniqueName = getUniqueName(result.name, existingDbs);
            useDatabaseStore.getState().addDatabase(result.id, { ...result, name: uniqueName });
            uiStore.showToast(`CSV 数据导入成功: ${result.rowCount} 行`, 'success');
          }
        } catch (error) {
          console.error('Failed to import CSV:', error);
          uiStore.showToast(`CSV 导入失败: ${error}`, 'error');
        }
      } else {
        uiStore.showToast(`${String(type).toUpperCase()} 导入功能开发中...`, 'warning');
      }
    },
  });
}

// database
export function useDatabaseManagement() {
  const selectedItemId = useEditorStore((s) => s.selectedItemId);
  const setSelectedInfo = useEditorStore((s) => s.setSelectedInfo);

  const updateDataFrame = useCallback((id: string, data: any) => {
    useDatabaseStore.getState().updateDatabase(id, data);
  }, []);

  const deleteDataFrame = useCallback((id: string) => {
    useDatabaseStore.getState().deleteDatabase(id);
    if (selectedItemId === id) setSelectedInfo(null, null);
    DatabaseService.deleteDatabase(id).catch((e) =>
      console.warn('[useDatabaseManagement] deleteDatabase backend failed:', e)
    );
  }, [selectedItemId, setSelectedInfo]);

  return {
    triggerImportData,
    updateDataFrame,
    deleteDataFrame,
  };
}
