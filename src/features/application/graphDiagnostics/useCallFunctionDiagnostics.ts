import { useMemo } from 'react';
import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';
import { useResourceStore } from '@/features/core/resource';
import {
  countCallFunctionIssuesByGraph,
  getCallFunctionIssueForNode,
  type CallFunctionIssue,
} from '@/features/domain/graphDiagnostics';

export function useCallFunctionIssueCountsByGraph(): Record<string, number> {
  const graphEntities = useGraphDataStore((s) => s.graphEntities);
  const resources = useResourceStore((s) => s.resources);

  return useMemo(
    () => countCallFunctionIssuesByGraph(graphEntities, resources),
    [graphEntities, resources],
  );
}

export function useCallFunctionIssue(
  graphPath: string | undefined,
  nodeId: string,
): CallFunctionIssue | null {
  const node = useGraphDataStore((s) => (graphPath ? s.getGraphNode(graphPath, nodeId) : undefined));
  const resources = useResourceStore((s) => s.resources);

  return useMemo(() => {
    if (!graphPath || !node) return null;
    return getCallFunctionIssueForNode(graphPath, node, resources);
  }, [graphPath, node, resources]);
}
