import { useMemo } from 'react';
import { useEditorCollections } from '@/features/core/editor';
import { useEditorSessionCommandsContext } from './EditorSessionContext';
import type {
  EditorSessionDetailActionsSlice,
  EditorSessionResourcesSlice,
} from './editorSessionTypes';

/** Resource consumers subscribe where the collections are actually used. */
export function useEditorSessionResources(): EditorSessionResourcesSlice {
  return useEditorCollections();
}

/** Detail 面板：变量 / DataFrame 更新 */
export function useEditorSessionDetailActions(): EditorSessionDetailActionsSlice {
  const commands = useEditorSessionCommandsContext();
  return useMemo(
    () => ({
      updateVariable: commands.updateVariable,
      updateDataFrame: commands.updateDataFrame,
    }),
    [commands.updateVariable, commands.updateDataFrame],
  );
}
