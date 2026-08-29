import { useCallback, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import {
  useEditorSessionCommandsContext,
  useEditorSessionResources,
} from '@/features/application/editor';
import { resolveActiveProjectGraph } from '@/features/application/sidebar';
import { updateVariableAction } from '@/features/application/dataManagement/variableActions';
import { renameResource } from '@/features/application/resource/resourceActions';
import {
  renameWorksheetResource,
  revealProjectResourceInExplorer,
} from '@/features/application/sidebar/sidebarResourceActions';
import { deleteWorksheetWithConfirm } from '@/features/application/editor/worksheetDelete';
import { openDatabaseEditorWindow } from '@/features/application/window';
import { ui } from '@/features/core/ui/ui';
import { useGraphSessionUi } from '@/features/core/graphSession/ui';
import { useVariableRead } from '@/features/core/variable/read';
import { useDatabaseRead } from '@/features/core/database/read';
import { getActiveLayoutTab } from '@/features/core/layout';
import type { GraphResourceType } from '../sidebarContextMenu/sidebarContextMenuTypes';

type OpenInputDialog = (
  title: string,
  value: string,
  onSubmit: (value: string) => void | Promise<void>,
  submitLabel?: string,
) => void;

export function useSidebarResourceActions(openInputDialog: OpenInputDialog) {
  const { t } = useTranslation();
  const { events, functions } = useEditorSessionResources();
  const focusedSession = useGraphSessionUi((snapshot) => snapshot.focusedSession);
  const variables = useVariableRead((snapshot) => snapshot.variables);
  const databases = useDatabaseRead((snapshot) => snapshot.databases);
  const activeTab = focusedSession
    ? getActiveLayoutTab(focusedSession.groupId)?.tab ?? null
    : null;
  const activeProjectGraph = useMemo(
    () => resolveActiveProjectGraph({ events, functions, activeTab }),
    [activeTab, events, functions],
  );
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
    const confirmed = await ui.confirm({
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
    if (!activeProjectGraph) return;
    const scope =
      activeProjectGraph.kind === 'function'
        ? { type: 'function' as const, functionPath: activeProjectGraph.path }
        : { type: 'event' as const, eventPath: activeProjectGraph.path };
    await updateVariableAction(id, { scope });
  }, [activeProjectGraph]);

  const addProjectVariable = useCallback(async (
    name?: string,
    type: string = 'Int64',
    isGlobal: boolean = false,
  ) => {
    if (!isGlobal && !activeProjectGraph) return null;
    return addVariable(
      name,
      type,
      isGlobal,
      activeProjectGraph
        ? {
            graphScope: {
              graphPath: activeProjectGraph.path,
              graphType: activeProjectGraph.kind,
            },
          }
        : undefined,
    );
  }, [activeProjectGraph, addVariable]);

  const renameDatabaseItem = useCallback((id: string, name: string) => {
    openInputDialog(t('contextMenu.dialog.renameDataTitle'), name, async (nextName) => {
      await renameResource({ id, kind: 'database' }, nextName);
    }, t('contextMenu.dialog.renameSubmit'));
  }, [openInputDialog, t]);

  const deleteDatabaseItem = useCallback(async (id: string, name: string) => {
    const confirmed = await ui.confirm({
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
    const variable = variables[id];
    const isGlobal = variable?.scope.type === 'global';
    return { type: 'variable' as const, id, name, isGlobal };
  }, [variables]);

  const resolveDatabaseName = useCallback((id: string, fallback: string) => {
    return databases[id]?.name ?? fallback;
  }, [databases]);

  return {
    renameGraphItem,
    deleteGraphItem,
    duplicateGraphItem,
    renameVariableItem,
    deleteVariableItem,
    promoteVariable,
    demoteVariable,
    canDemoteVariable: activeProjectGraph !== null,
    renameDatabaseItem,
    deleteDatabaseItem,
    renameWorksheetItem,
    deleteWorksheetItem,
    revealInExplorer,
    openVariableContextMenuTarget,
    resolveDatabaseName,
    addVariable: addProjectVariable,
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
