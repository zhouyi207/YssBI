import { useLayoutStore } from '@/features/core/layout';
import { getGraphById } from '@/features/core/dataStore';

export function resolveExecutionGraphId(targetGraphId?: string): string | undefined {
  if (targetGraphId) return targetGraphId;

  const layoutStore = useLayoutStore.getState();
  const editorGroupId = layoutStore.activeEditorGroupId || layoutStore.activeGroupId;
  const editorNode = editorGroupId ? layoutStore.nodes[editorGroupId] : null;
  return editorNode?.data?.activeTabId as string | undefined;
}

export function getExecutionEventGraph(targetGraphId?: string) {
  const graphId = resolveExecutionGraphId(targetGraphId);
  if (!graphId) return null;
  const graph = getGraphById(graphId);
  if (!graph || graph.type !== 'event') return null;
  return { graphId, graph };
}
