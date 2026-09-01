import type { DiagnosticDto, DiagnosticLocationDto } from "@/shared/types/domain/editorProjection";
import type { DeepReadonly } from "@/shared/types/deepReadonly";
import type { ConnectionData, NodeData, PinData } from "@/shared/types/store/graph";
import {
  formatNodePinDisplayLabel,
  nodeDisplayTitle,
  resolveNodePinDisplayLabel,
} from "@/features/domain/editorProjection";

export interface GraphNodeDiagnosticsBucket {
  readonly graphNodes: readonly string[];
  readonly nodes: Readonly<
    Record<
      string,
      Pick<DeepReadonly<NodeData>, "title" | "display" | "diagnostics" | "parameterEditors">
    >
  >;
  readonly pins?: Readonly<Record<string, Pick<PinData, "name" | "display">>>;
  readonly connections?: Readonly<Record<string, Pick<ConnectionData, "output" | "input">>>;
}

export interface GraphNodeDiagnostic {
  readonly graphPath: string;
  readonly nodeId: string;
  readonly nodeTitle: string;
  readonly locationLabel: string;
  readonly diagnostic: DeepReadonly<DiagnosticDto>;
}

export function formatDiagnosticLocationLabel(
  location: DiagnosticLocationDto,
  bucket: GraphNodeDiagnosticsBucket | undefined,
  ownerNodeId: string,
): string | null {
  const ownerTitle = nodeDisplayTitle(bucket?.nodes[ownerNodeId]);

  switch (location.kind) {
    case "graph":
    case "resource":
      return ownerTitle;
    case "node":
      return nodeDisplayTitle(bucket?.nodes[location.nodeId]) ?? ownerTitle;
    case "port":
      return resolveNodePinDisplayLabel(bucket, location.address) ?? ownerTitle;
    case "parameter": {
      const nodeTitle = nodeDisplayTitle(bucket?.nodes[location.nodeId]) ?? ownerTitle;
      const parameterTitle = bucket?.nodes[location.nodeId]?.parameterEditors?.find(
        (parameter) => parameter.key === location.key,
      )?.display.title;
      return formatNodePinDisplayLabel(nodeTitle, parameterTitle) ?? ownerTitle;
    }
    case "connection": {
      const connection = bucket?.connections?.[location.connectionId];
      if (!connection || !connection.output || !connection.input) return ownerTitle;
      const output = resolveNodePinDisplayLabel(bucket, connection.output);
      const input = resolveNodePinDisplayLabel(bucket, connection.input);
      if (output && input) return `${output} → ${input}`;
      return output ?? input ?? ownerTitle;
    }
    default:
      return null;
  }
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
      nodeTitle: nodeDisplayTitle(node) ?? "",
      locationLabel:
        formatDiagnosticLocationLabel(diagnostic.location, bucket, nodeId) ??
        nodeDisplayTitle(node) ??
        "",
      diagnostic,
    }));
  });
}
