export type NodeInspectionTarget =
  | { kind: 'empty' }
  | { kind: 'multiple'; count: number }
  | { kind: 'node'; graphPath: string; nodeId: string };

export function resolveNodeInspectionTarget(
  graphPath: string | null,
  selectedNodeIds: readonly string[],
): NodeInspectionTarget {
  if (!graphPath || selectedNodeIds.length === 0) return { kind: 'empty' };
  if (selectedNodeIds.length > 1) {
    return { kind: 'multiple', count: selectedNodeIds.length };
  }
  return { kind: 'node', graphPath, nodeId: selectedNodeIds[0] };
}
