import { useRef } from 'react';
import { useActiveEditorGroup, useEditorActions } from '@/features/core/editor';
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
import {
  pickEditorSessionLayoutBindings,
  pickEditorSessionNodeActions,
} from './editorSessionTypes';
import {
  createEditorSessionCommandsContainer,
  patchEditorSessionCommands,
  type EditorSessionCommands,
} from './editorSessionCommands';

/**
 * Builds the command surface once per provider mount.
 * The returned object identity is stable; fields are patched each render.
 */
export function useEditorSessionCommands(): EditorSessionCommands {
  const active = useActiveEditorGroup();
  const actions = useEditorActions(active);

  const editorOps = useEditorOperations();
  const tabMgmt = useTabManagement();
  const openWorksheet = useOpenWorksheet();
  const worksheetMgmt = useWorksheetManagement(openWorksheet);
  const projectOps = useProjectOperations();

  const graphMgmt = useGraphManagement(tabMgmt.openGraph);
  const variableMgmt = useVariableManagement();
  const dataFrameMgmt = useDatabaseManagement();
  const nodeMgmt = useNodeManagement();

  const layoutBindings = pickEditorSessionLayoutBindings(actions);
  const nodeActions = pickEditorSessionNodeActions(nodeMgmt);

  const containerRef = useRef<EditorSessionCommands | null>(null);
  if (!containerRef.current) {
    containerRef.current = createEditorSessionCommandsContainer();
  }

  return patchEditorSessionCommands(containerRef.current, {
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
  });
}
