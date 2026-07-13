import { useMemo } from 'react';
import { useEditorSessionSharedContext, useEditorSessionCommandsContext } from './EditorSessionContext';
import type {
  EditorSessionDetailActionsSlice,
  EditorSessionResourcesSlice,
} from './editorSessionTypes';
import { pickEditorSessionResources } from './editorSessionTypes';

/** Detail / 侧栏资源列表：仅暴露 collections 四表 */
export function useEditorSessionResources(): EditorSessionResourcesSlice {
  const shared = useEditorSessionSharedContext();
  return useMemo(
    () => pickEditorSessionResources(shared as Parameters<typeof pickEditorSessionResources>[0]),
    [shared.events, shared.functions, shared.variables, shared.dataframes],
  );
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
