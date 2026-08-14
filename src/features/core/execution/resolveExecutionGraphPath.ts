import { editorDockviewPort } from '@/features/core/dockview';
import type { LayoutTab } from '@/shared/types';
import { getGraphByPath } from '@/features/core/dataStore';

export function resolveExecutionGraphPath(targetGraphPath?: string): string | undefined {
  if (targetGraphPath) return targetGraphPath;

  const value = editorDockviewPort.getActivePanel()?.tab?.data?.layoutTab;
  if (!value || typeof value !== 'object') return undefined;
  return (value as LayoutTab).id;
}

export function getExecutionEventGraph(targetGraphPath?: string) {
  const graphPath = resolveExecutionGraphPath(targetGraphPath);
  if (!graphPath) return null;
  const graph = getGraphByPath(graphPath);
  if (!graph || graph.type !== 'event') return null;
  return { graphPath, graph };
}
