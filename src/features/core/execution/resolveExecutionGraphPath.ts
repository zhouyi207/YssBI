import { useLayoutStore } from '@/features/core/layout';
import { getEditorGroupActiveTabId } from '@/features/core/layout/editorTabStore';
import { getGraphByPath } from '@/features/core/dataStore';

export function resolveExecutionGraphPath(targetGraphPath?: string): string | undefined {
  if (targetGraphPath) return targetGraphPath;

  const layoutStore = useLayoutStore.getState();
  const editorGroupId = layoutStore.activeEditorGroupId;
  if (!editorGroupId) return undefined;
  return getEditorGroupActiveTabId(editorGroupId) ?? undefined;
}

export function getExecutionEventGraph(targetGraphPath?: string) {
  const graphPath = resolveExecutionGraphPath(targetGraphPath);
  if (!graphPath) return null;
  const graph = getGraphByPath(graphPath);
  if (!graph || graph.type !== 'event') return null;
  return { graphPath, graph };
}
