import { getGraphByPath } from '@/features/application/project/projectHelpers';
import { workbenchDockviewRead } from '@/features/core/dockview/workbenchRead';

export function resolveExecutionGraphPath(targetGraphPath?: string): string | undefined {
  if (targetGraphPath) return targetGraphPath;

  const panel = workbenchDockviewRead.getActiveEditorPanel();
  return panel?.metadata.role === 'editor'
    ? panel.metadata.resourceRef
    : undefined;
}

export function getExecutionEventGraph(targetGraphPath?: string) {
  const graphPath = resolveExecutionGraphPath(targetGraphPath);
  if (!graphPath) return null;
  const graph = getGraphByPath(graphPath);
  if (!graph || graph.type !== 'event') return null;
  return { graphPath, graph };
}
