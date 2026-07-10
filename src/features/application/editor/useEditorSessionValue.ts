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
import type { EditorSession } from './editorSessionTypes';
import {
  pickEditorSessionLayoutBindings,
  pickEditorSessionNodeActions,
} from './editorSessionTypes';

/**
 * Builds the shared editor session once per Editor window.
 * Canvas pointer interaction is mounted separately via useEditorGroup({ withCanvasInteraction: true }).
 */
export function useEditorSessionValue(): EditorSession {
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

  const layoutBindings = useMemo(
    () => pickEditorSessionLayoutBindings(actions),
    [actions],
  );

  const nodeActions = useMemo(
    () => pickEditorSessionNodeActions(nodeMgmt),
    [nodeMgmt],
  );

  return useMemo(
    (): EditorSession => ({
      ...state,
      ...layoutBindings,
      ...editorOps,
      ...tabMgmt,
      openWorksheet,
      ...worksheetMgmt,
      ...projectOps,
      ...graphMgmt,
      ...variableMgmt,
      ...dataFrameMgmt,
      ...nodeActions,
    }),
    [
      state,
      layoutBindings,
      editorOps,
      tabMgmt,
      openWorksheet,
      worksheetMgmt,
      projectOps,
      graphMgmt,
      variableMgmt,
      dataFrameMgmt,
      nodeActions,
    ],
  );
}
