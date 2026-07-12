import { useMemo } from 'react';
import { useEditorSession } from './EditorSessionContext';
import type {
  EditorSessionDetailActionsSlice,
  EditorSessionResourcesSlice,
} from './editorSessionTypes';
import { pickEditorSessionResources } from './editorSessionTypes';

/** Detail / 侧栏资源列表：仅暴露 collections 四表 */
export function useEditorSessionResources(): EditorSessionResourcesSlice {
  const session = useEditorSession();
  return useMemo(
    () => pickEditorSessionResources(session),
    [session.events, session.functions, session.variables, session.dataframes],
  );
}

/** Detail 面板：变量 / DataFrame 更新 */
export function useEditorSessionDetailActions(): EditorSessionDetailActionsSlice {
  const session = useEditorSession();
  return useMemo(
    () => ({
      updateVariable: session.updateVariable,
      updateDataFrame: session.updateDataFrame,
    }),
    [session.updateVariable, session.updateDataFrame],
  );
}
