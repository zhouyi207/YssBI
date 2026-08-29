import type { DiagnosticDto } from '@/shared/types/domain/editorProjection';
import type { DeepReadonly } from '@/shared/types/deepReadonly';
import type { NodeData } from '@/shared/types/store/graph';

export interface GraphNodeDiagnosticsBucket {
  readonly graphNodes: readonly string[];
  readonly nodes: Readonly<
    Record<string, Pick<DeepReadonly<NodeData>, 'title' | 'display' | 'diagnostics'>>
  >;
}

export interface GraphNodeDiagnostic {
  readonly graphPath: string;
  readonly nodeId: string;
  readonly nodeTitle: string;
  readonly diagnostic: DeepReadonly<DiagnosticDto>;
}

export function collectNodeDiagnostics(
  graphPath: string,
  bucket: GraphNodeDiagnosticsBucket | undefined,
): GraphNodeDiagnostic[] {
  if (!bucket) return [];

  return bucket.graphNodes.flatMap((nodeId) => {
    const node = bucket.nodes[nodeId];
    if (!node) return [];

    return (node.diagnostics ?? []).map((diagnostic) => ({
      graphPath,
      nodeId,
      nodeTitle: node.display?.title ?? node.title,
      diagnostic,
    }));
  });
}
