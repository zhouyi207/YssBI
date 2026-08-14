import { logger } from "@/utils/appLogger";
import { useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { useEditorSessionCommandsContext } from '@/features/application/editor';
import { updateVariableAction } from '@/features/application/dataManagement/variableActions';
import { renameResource } from '@/features/application/resource/resourceActions';
import {
  renameWorksheetResource,
  revealProjectResourceInExplorer,
} from '@/features/application/sidebar/sidebarResourceActions';
import { deleteWorksheetWithConfirm } from '@/features/application/editor/closeEditorTab';
import { openDatabaseEditorWindow } from '@/features/application/window';
import { uiStore } from '@/features/core/ui/UIStore';
import { useVariableStore } from '@/features/core/dataStore/variableStore';
import { useDatabaseStore } from '@/features/core/dataStore/databaseStore';
import type { GraphResourceType } from '../sidebarContextMenu/sidebarContextMenuTypes';
import { useSidebarVariableScope } from './useSidebarVariableScope';

type OpenInputDialog = (
  title: string,
  value: string,
  onSubmit: (value: string) => void | Promise<void>,
  submitLabel?: string,
) => void;

export function useSidebarResourceActions(openInputDialog: OpenInputDialog) {
  const { t } = useTranslation();
  const { scopePath, graphType } = useSidebarVariableScope();
  const {
    renameGraph,
    duplicateGraph,
    deleteEvent,
    deleteFunction,
    deleteVariable,
    deleteDataFrame,
    addVariable,
    addEvent,
    addFunction,
    createGraph,
    openGraph,
    openWorksheet,
    duplicateWorksheet,
    addWorksheet,
    triggerImportData,
  } = useEditorSessionCommandsContext();

  const renameGraphItem = useCallback((id: string, name: string, type: GraphResourceType) => {
    openInputDialog(t('contextMenu.dialog.renameGraphTitle'), name, async (nextName) => {
      await renameGraph(id, nextName, type);
    }, t('contextMenu.dialog.renameSubmit'));
  }, [openInputDialog, renameGraph, t]);

  const deleteGraphItem = useCallback(async (id: string, type: GraphResourceType) => {
    if (type === 'event') {
      await deleteEvent(id);
      return;
    }
    await deleteFunction(id);
  }, [deleteEvent, deleteFunction]);

  const duplicateGraphItem = useCallback(async (id: string) => {
    await duplicateGraph(id);
  }, [duplicateGraph]);

  const renameVariableItem = useCallback((id: string, name: string) => {
    openInputDialog(t('contextMenu.dialog.renameVariableTitle'), name, async (nextName) => {
      await renameResource({ id, kind: 'variable' }, nextName);
    }, t('contextMenu.dialog.renameSubmit'));
  }, [openInputDialog, t]);

  const deleteVariableItem = useCallback(async (id: string, name: string) => {
    const confirmed = await uiStore.confirm({
      title: t('sidebar.deleteVariableTitle'),
      message: t('sidebar.deleteVariableMessage', { name }),
      confirmText: t('contextMenu.sidebar.delete'),
      cancelText: t('common.cancel'),
      type: 'danger',
    });
    if (!confirmed) return;
    await deleteVariable(id);
  }, [deleteVariable, t]);

  const promoteVariable = useCallback(async (id: string) => {
    await updateVariableAction(id, { scope: { type: 'global' } });
  }, []);

  const demoteVariable = useCallback(async (id: string) => {
    if (!scopePath || !graphType) {
      logger.notify.warn(t('sidebar.noActiveGraph'), "UI");
      return;
    }
    const scope =
      graphType === 'function'
        ? { type: 'function' as const, functionPath: scopePath }
        : { type: 'event' as const, eventPath: scopePath };
    await updateVariableAction(id, { scope });
  }, [graphType, scopePath, t]);

  const renameDatabaseItem = useCallback((id: string, name: string) => {
    openInputDialog(t('contextMenu.dialog.renameDataTitle'), name, async (nextName) => {
      await renameResource({ id, kind: 'database' }, nextName);
    }, t('contextMenu.dialog.renameSubmit'));
  }, [openInputDialog, t]);

  const deleteDatabaseItem = useCallback(async (id: string, name: string) => {
    const confirmed = await uiStore.confirm({
      title: t('sidebar.deleteDataTitle'),
      message: t('sidebar.deleteDataMessage', { name }),
      confirmText: t('contextMenu.sidebar.delete'),
      cancelText: t('common.cancel'),
      type: 'danger',
    });
    if (!confirmed) return;
    await deleteDataFrame(id);
  }, [deleteDataFrame, t]);

  const renameWorksheetItem = useCallback((worksheetPath: string, name: string) => {
    openInputDialog(t('contextMenu.dialog.renameWorksheetTitle'), name, async (nextName) => {
      await renameWorksheetResource(worksheetPath, nextName);
    }, t('contextMenu.dialog.renameSubmit'));
  }, [openInputDialog, t]);

  const deleteWorksheetItem = useCallback(async (worksheetPath: string) => {
    await deleteWorksheetWithConfirm(worksheetPath);
  }, []);

  const revealInExplorer = useCallback(async (request: Parameters<typeof revealProjectResourceInExplorer>[0]) => {
    await revealProjectResourceInExplorer(request);
  }, []);

  const openVariableContextMenuTarget = useCallback((id: string, name: string) => {
    const variable = useVariableStore.getState().variables[id];
    const isGlobal = variable?.scope.type === 'global';
    return { type: 'variable' as const, id, name, isGlobal };
  }, []);

  const resolveDatabaseName = useCallback((id: string, fallback: string) => {
    return useDatabaseStore.getState().databases[id]?.name ?? fallback;
  }, []);

  return {
    renameGraphItem,
    deleteGraphItem,
    duplicateGraphItem,
    renameVariableItem,
    deleteVariableItem,
    promoteVariable,
    demoteVariable,
    renameDatabaseItem,
    deleteDatabaseItem,
    renameWorksheetItem,
    deleteWorksheetItem,
    revealInExplorer,
    openVariableContextMenuTarget,
    resolveDatabaseName,
    addVariable,
    addEvent,
    addFunction,
    createGraph,
    openGraph,
    openWorksheet,
    duplicateWorksheet,
    addWorksheet,
    triggerImportData,
    openDatabaseEditorWindow,
  };
}
