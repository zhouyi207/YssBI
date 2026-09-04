import type {
  DiagnosticDto,
  DiagnosticLocationDto,
  PortAddressDto,
} from "@/shared/types/domain/editorProjection";
import type { DeepReadonly } from "@/shared/types/deepReadonly";
import type {
  ConnectionData,
  NodeData,
  PinData,
} from "@/features/domain/editorProjection/graphRuntimeTypes";
import {
  formatNodePinDisplayLabel,
  nodeDisplayTitle,
  portAddressKey,
  resolveNodePinDisplayLabel,
} from "@/features/domain/editorProjection";

export interface GraphNodeDiagnosticsBucket {
  readonly graphNodes: readonly string[];
  readonly nodes: Readonly<
    Record<string, Pick<DeepReadonly<NodeData>, "display" | "diagnostics" | "parameterEditors">>
  >;
  readonly pins?: Readonly<Record<string, Pick<PinData, "name" | "display">>>;
  readonly connections?: Readonly<Record<string, Pick<ConnectionData, "output" | "input">>>;
}

export interface GraphProblemsBucket extends GraphNodeDiagnosticsBucket {
  readonly sourceRevision: number;
  readonly diagnostics: readonly DeepReadonly<DiagnosticDto>[];
}

export interface GraphProblem {
  readonly graphPath: string;
  readonly sourceRevision: number;
  readonly nodeId: string | null;
  readonly locationLabel: string;
  readonly diagnostic: DeepReadonly<DiagnosticDto>;
}

export function findPrimaryPortDiagnostic(
  diagnostics: readonly DeepReadonly<DiagnosticDto>[],
  address: PortAddressDto,
): DeepReadonly<DiagnosticDto> | undefined {
  const addressKey = portAddressKey(address);
  const matches = diagnostics.filter(
    (diagnostic) =>
      diagnostic.location.kind === "port" &&
      portAddressKey(diagnostic.location.address) === addressKey,
  );
  return matches.find((diagnostic) => diagnostic.blocking) ?? matches[0];
}

export function isUnboundInputDiagnostic(
  diagnostic: { readonly code: string } | undefined,
): boolean {
  return (
    diagnostic?.code === "compiler.input.unbound" || diagnostic?.code === "node.input.not_connected"
  );
}

export function formatDiagnosticLocationLabel(
  location: DiagnosticLocationDto,
  bucket: GraphNodeDiagnosticsBucket | undefined,
  ownerNodeId: string | null,
): string | null {
  const ownerTitle = ownerNodeId ? (nodeDisplayTitle(bucket?.nodes[ownerNodeId]) ?? null) : null;

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

function nodeIdFromDiagnosticLocation(location: DiagnosticLocationDto): string | null {
  switch (location.kind) {
    case "node":
    case "parameter":
      return location.nodeId;
    case "port":
      return location.address.nodeId;
    case "graph":
    case "resource":
    case "connection":
      return null;
  }
}

export function collectGraphProblems(
  graphPath: string,
  bucket: GraphProblemsBucket | undefined,
): GraphProblem[] {
  if (!bucket) return [];

  return bucket.diagnostics.map((diagnostic) => {
    const nodeId = nodeIdFromDiagnosticLocation(diagnostic.location);
    return {
      graphPath,
      sourceRevision: bucket.sourceRevision,
      nodeId,
      locationLabel:
        formatDiagnosticLocationLabel(diagnostic.location, bucket, nodeId) ?? graphPath,
      diagnostic,
    };
  });
}
