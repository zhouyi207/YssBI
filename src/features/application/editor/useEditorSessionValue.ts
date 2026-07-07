import { useMemo } from 'react';
import {
  useActiveEditorGroup,
  useEditorCollections,
  useEditorGroups,
  useEditorUIState,
  useEditorActions,
  buildEditorState,
} from '@/features/core/editor';
import { useEditorOperations } from './useEditorOperations';
import { useTabManagement } from './useTabManagement';
import { useOpenWorksheet, useWorksheetManagement } from './useWorksheetManagement';
import { useProjectOperations } from './useProjectOperations';
import {
  useGraphManagement,
  useVariableManagement,
  useDatabaseManagement,
  useNodeManagement,
} from '@/features/application/dataManagement';

/**
 * Builds the shared editor session once per Editor window.
 * Canvas pointer interaction is mounted separately via useEditorGroup({ withCanvasInteraction: true }).
 */
export function useEditorSessionValue() {
  const active = useActiveEditorGroup();
  const collections = useEditorCollections();
  const groups = useEditorGroups();
  const uiState = useEditorUIState();
  const actions = useEditorActions(active);

  const state = useMemo(
    () => buildEditorState(active, collections, groups, uiState),
    [active, collections, groups, uiState],
  );

  const editorOps = useEditorOperations();
  const tabMgmt = useTabManagement();
  const openWorksheet = useOpenWorksheet();
  const worksheetMgmt = useWorksheetManagement(openWorksheet);
  const projectOps = useProjectOperations();

  const graphMgmt = useGraphManagement(tabMgmt.openGraph);
  const variableMgmt = useVariableManagement();
  const dataFrameMgmt = useDatabaseManagement();
  const nodeMgmt = useNodeManagement();

  return useMemo(
    () => ({
      ...state,
      setNodes: actions.setNodes,
      setCanvas: actions.setCanvas,
      setActiveGroupId: actions.setActiveGroupId,
      setContextMenu: actions.setContextMenu,
      setDetailFocus: actions.setDetailFocus,
      clearDetailFocus: actions.clearDetailFocus,
      setPendingConnection: actions.setPendingConnection,
      activeGroupIdRef: actions.activeGroupIdRef,
      activeTabIdRef: actions.activeTabIdRef,
      viewportRef: actions.viewportRef,
      ...editorOps,
      ...tabMgmt,
      openWorksheet,
      ...worksheetMgmt,
      ...projectOps,
      ...graphMgmt,
      ...variableMgmt,
      ...dataFrameMgmt,
      createNode: nodeMgmt.createNode,
      createNodes: nodeMgmt.createNodes,
      deleteNode: nodeMgmt.deleteNode,
      deleteNodes: nodeMgmt.deleteNodes,
      handleNodeCreated: nodeMgmt.handleNodeCreated,
      handleNodeDeleted: nodeMgmt.handleNodeDeleted,
    }),
    [
      state,
      actions,
      editorOps,
      tabMgmt,
      openWorksheet,
      worksheetMgmt,
      projectOps,
      graphMgmt,
      variableMgmt,
      dataFrameMgmt,
      nodeMgmt,
    ],
  );
}

export type EditorSession = ReturnType<typeof useEditorSessionValue>;
