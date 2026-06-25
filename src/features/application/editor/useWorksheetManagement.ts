import { useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { useEditorStore } from '@/features/core/editor';
import { useWorksheetStore } from '@/features/core/worksheet/worksheetStore';
import { WorksheetService } from '@/services/worksheet/worksheetService';
import { ensureDetailVisible } from './ensureDetailVisible';
import { useSidebarTab } from './useSidebarTab';
import { uiStore } from '@/features/core/ui/UIStore';

export function useWorksheetManagement(openWorksheet: (id: string, name: string) => Promise<void>) {
  const { t } = useTranslation();
  const switchSidebarTab = useSidebarTab();

  const addWorksheet = useCallback(
    async (databaseId?: string) => {
      try {
        const document = await WorksheetService.createWorksheet(
          t('worksheet.defaultName'),
          databaseId,
        );
        useWorksheetStore.getState().upsertDocument(document);
        switchSidebarTab('charts');
        await openWorksheet(document.id, document.name);
        uiStore.showToast(t('worksheet.created'), 'success', 2000);
      } catch (error) {
        uiStore.showToast(
          `${t('worksheet.createFailed')}: ${error instanceof Error ? error.message : String(error)}`,
          'error',
          4000,
        );
      }
    },
    [openWorksheet, switchSidebarTab, t],
  );

  const ensureWorksheetLoaded = useCallback(async (worksheetId: string) => {
    const cached = useWorksheetStore.getState().documents[worksheetId];
    if (cached) return cached;
    const document = await WorksheetService.loadWorksheet(worksheetId);
    useWorksheetStore.getState().upsertDocument(document);
    return document;
  }, []);

  return { addWorksheet, ensureWorksheetLoaded };
}

export function useOpenWorksheet() {
  const setSelectedInfo = useEditorStore((s) => s.setSelectedInfo);
  const switchSidebarTab = useSidebarTab();

  return useCallback(async (id: string, name: string) => {
    const layoutStore = useLayoutStore.getState();
    const targetGroupId =
      layoutStore.activeEditorGroupId || layoutStore.activeGroupId || 'default_editor';

    if (!useWorksheetStore.getState().documents[id]) {
      try {
        const loaded = await WorksheetService.loadWorksheet(id);
        useWorksheetStore.getState().upsertDocument(loaded);
      } catch {
        // Index-only open: WorksheetEditor will retry load on mount.
      }
    }

    layoutStore.addTab(targetGroupId, {
      id,
      title: name,
      component: 'WorksheetEditor',
      type: 'worksheet',
    });

    layoutStore.setActiveGroup(targetGroupId);
    ensureDetailVisible();
    switchSidebarTab('charts');
    setSelectedInfo(id, 'worksheet');
  }, [setSelectedInfo, switchSidebarTab]);
}
