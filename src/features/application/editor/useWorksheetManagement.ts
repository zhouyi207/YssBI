import { useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { DEFAULT_WORKSHEET_NAME } from '@/shared/constants/defaultResourceNames';
import { useWorksheetStore } from '@/features/core/worksheet/worksheetStore';
import { commitFileFirstResourceIndex } from '@/features/application/resource/resourceActions';
import { WorksheetService } from '@/services/worksheet/worksheetService';
import { projectPublicationCoordinator } from '@/features/application/editorMutation/projectPublicationCoordinator';
import { captureProjectCommandContext } from '@/features/application/projectCommandContext';

import { uiStore } from '@/features/core/ui/UIStore';
import { useSidebarTab } from './useSidebarTab';
import { buildWorksheetLayoutTab } from '@/features/core/layout/layoutTabModel';
import { openEditorTab } from './openEditorTab';

export function useWorksheetManagement(openWorksheet: (id: string, name: string) => Promise<void>) {
  const { t } = useTranslation();
  const switchSidebarTab = useSidebarTab();

  const addWorksheet = useCallback(
    async (databaseId?: string) => {
      let context: ReturnType<typeof captureProjectCommandContext> | undefined;
      try {
        context = captureProjectCommandContext();
        const created = await WorksheetService.createWorksheet(
          context.projectInstanceId,
          context.operationId,
          DEFAULT_WORKSHEET_NAME,
          databaseId,
        );
        if (!context.isCurrent()) return;
        await projectPublicationCoordinator.submit({ result: created.result });
        if (!context.isCurrent()) return;

        const { document } = created;
        await commitFileFirstResourceIndex();
        if (!context.isCurrent()) return;
        switchSidebarTab('charts');
        await openWorksheet(document.id, document.name);
        if (!context.isCurrent()) return;
        uiStore.showToast(t('worksheet.created'), 'success', 2000);
      } catch (error) {
        if (context && !context.isCurrent()) return;
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
    const context = captureProjectCommandContext();
    const document = await WorksheetService.loadWorksheet(context.projectInstanceId, worksheetId);
    if (!context.isCurrent()) return null;
    useWorksheetStore.getState().upsertDocument(document);
    return document;
  }, []);

  return { addWorksheet, ensureWorksheetLoaded };
}

export function useOpenWorksheet() {
  const switchSidebarTab = useSidebarTab();

  return useCallback(async (id: string, _name: string) => {
    if (!useWorksheetStore.getState().documents[id]) {
      const context = captureProjectCommandContext();
      try {
        const loaded = await WorksheetService.loadWorksheet(context.projectInstanceId, id);
        if (!context.isCurrent()) return;
        useWorksheetStore.getState().upsertDocument(loaded);
      } catch {
        if (!context.isCurrent()) return;
        // Index-only open: WorksheetEditor retries load on mount.
      }
    }

    openEditorTab(buildWorksheetLayoutTab(id), { focusDetail: { kind: 'worksheet', id } });

    switchSidebarTab('charts');
  }, [switchSidebarTab]);
}
