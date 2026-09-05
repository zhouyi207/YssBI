import type {
  DiagnosticDto,
  DiagnosticLocationDto,
  PortAddressDto,
} from "@/shared/types/domain/editorProjection";
import type { DeepReadonly } from "@/shared/types/deepReadonly";
import diagnosticTemplates from "./diagnosticTemplates.generated.json";
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
  readonly diagnostics: readonly DeepReadonly<DiagnosticDto>[];
}

export interface GraphProblem {
  readonly graphPath: string;
  readonly nodeId: string | null;
  readonly locationLabel: string;
  readonly diagnostic: DeepReadonly<DiagnosticDto>;
}

export function formatGraphDiagnostic(
  diagnostic: Pick<DeepReadonly<DiagnosticDto>, "code" | "messageKey" | "arguments">,
  language: string | undefined,
): string {
  const locale = language?.startsWith("zh") ? "zh-CN" : "en-US";
  const code = /^[a-z][a-z0-9_.]{0,127}$/u.test(diagnostic.code) ? diagnostic.code : "unknown";
  const fallback = locale === "zh-CN" ? `图问题（${code}）。` : `Graph problem (${code}).`;
  const template = Object.prototype.hasOwnProperty.call(diagnosticTemplates, diagnostic.messageKey)
    ? diagnosticTemplates[diagnostic.messageKey as keyof typeof diagnosticTemplates]
    : undefined;
  if (!template || template.code !== diagnostic.code) return fallback;
  if (
    template.parameters.some(
      (name) => !Object.prototype.hasOwnProperty.call(diagnostic.arguments, name),
    )
  )
    return fallback;
  return template[locale].replace(/\{([a-z_]+)\}/gu, (_, name: string) => {
    let text = "";
    for (const character of diagnostic.arguments[name]) {
      if (text.length + character.length > 512) break;
      const code = character.codePointAt(0)!;
      text += code < 32 || code === 127 ? " " : character;
    }
    return text;
  });
}

export function findPrimaryPortDiagnostic(
  diagnostics: readonly DeepReadonly<DiagnosticDto>[],
  address: PortAddressDto,
): DeepReadonly<DiagnosticDto> | undefined {
  const addressKey = portAddressKey(address);
  const matchingDiagnostics = diagnostics.filter(
    (diagnostic) =>
      diagnostic.location.kind === "port" &&
      portAddressKey(diagnostic.location.address) === addressKey,
  );
  return matchingDiagnostics.find((diagnostic) => diagnostic.blocking) ?? matchingDiagnostics[0];
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
      return ownerTitle;
    case "resource":
      return location.identity;
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
      const outputLabel = resolveNodePinDisplayLabel(bucket, connection.output);
      const inputLabel = resolveNodePinDisplayLabel(bucket, connection.input);
      if (outputLabel && inputLabel) return `${outputLabel} → ${inputLabel}`;
      return outputLabel ?? inputLabel ?? ownerTitle;
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
      nodeId,
      locationLabel:
        formatDiagnosticLocationLabel(diagnostic.location, bucket, nodeId) ?? graphPath,
      diagnostic,
    };
  });
}
