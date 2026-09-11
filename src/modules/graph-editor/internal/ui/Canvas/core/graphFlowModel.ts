import type { Node, Edge } from "@xyflow/react";
import type { GraphProjectionSnapshot } from "@/features/core/graph/read";
import type { PinData, ConnectionData } from "@/features/domain/editorProjection/graphRuntimeTypes";
import { toInteractionPinData } from "@/features/domain/editorProjection/interactionPinData";
import { resolveConnectionCompatibility } from "@/features/domain/editorProjection/connectionRules";

export type GraphFlowNode = Node<{ handlesKey: string }, "graph">;
export type GraphFlowEdge = Edge<{ fromPinId: string; toPinId: string }, "graph">;
export type FlowConnectionIntent = "connect" | "moveConnections";
export type FlowConnectionFeedback =
  | { kind: "append" }
  | { kind: "replace"; displacedConnectionIds: readonly string[] }
  | { kind: "invalid"; reason: string };

const REASONS = {
  samePort: "same-port",
  sameNode: "same-node",
  directionMismatch: "same-direction",
  typeMismatch: "type-mismatch",
  orphan: "orphan",
  capacityReached: "capacity",
} as const;

export function buildGraphFlowModel(
  bucket: GraphProjectionSnapshot["graphEntities"][string] | undefined,
) {
  const pins: Record<string, PinData> = {};
  for (const pin of Object.values(bucket?.pins ?? {})) pins[pin.id] = toInteractionPinData(pin);
  const nodes: GraphFlowNode[] = (bucket?.graphNodes ?? []).flatMap((id) => {
    const node = bucket?.nodes[id];
    return node
      ? [
          {
            id,
            type: "graph" as const,
            position: { ...node.position },
            data: { handlesKey: node.pinIds.join("\u0000") },
            selectable: !node.capabilities.managed,
            draggable: !node.capabilities.managed,
            deletable: false,
          },
        ]
      : [];
  });
  const nodeIds = new Set(nodes.map((node) => node.id));
  const connections = structuredClone(bucket?.connections ?? {}) as Record<string, ConnectionData>;
  const edges: GraphFlowEdge[] = Object.values(connections).flatMap((connection) => {
    const from = pins[connection.from],
      to = pins[connection.to];
    if (!from || !to || !nodeIds.has(from.nodeId) || !nodeIds.has(to.nodeId)) return [];
    return [
      {
        id: connection.id,
        type: "graph" as const,
        source: from.nodeId,
        target: to.nodeId,
        sourceHandle: from.id,
        targetHandle: to.id,
        data: { fromPinId: from.id, toPinId: to.id },
        deletable: false,
        reconnectable: false,
        selectable: false,
      },
    ];
  });
  return {
    nodes,
    nodeIds,
    edges,
    pins,
    connections,
    pinConnections: bucket?.pinConnections ?? {},
    draggableNodeIds: new Set(nodes.filter((node) => node.draggable).map((node) => node.id)),
  };
}

export type GraphFlowModel = ReturnType<typeof buildGraphFlowModel>;

export function resolveFlowConnection(
  model: GraphFlowModel,
  sourceId: string,
  targetId: string,
  intent: FlowConnectionIntent,
): FlowConnectionFeedback {
  const source = model.pins[sourceId],
    target = model.pins[targetId];
  if (!source || !target || source.orphan || target.orphan)
    return { kind: "invalid", reason: "orphan" };
  if (source.id === target.id) return { kind: "invalid", reason: "same-port" };
  if (intent === "connect") {
    const feedback = resolveConnectionCompatibility(source, target);
    if (feedback.kind === "invalid") return { kind: "invalid", reason: REASONS[feedback.reason] };
    if (feedback.kind === "append") return feedback;
    return {
      kind: "replace",
      displacedConnectionIds: [
        ...new Set([
          ...(source.connections.canReplace ? (model.pinConnections[source.id] ?? []) : []),
          ...(target.connections.canReplace ? (model.pinConnections[target.id] ?? []) : []),
        ]),
      ],
    };
  }
  // Moving links replaces one endpoint with another of the SAME direction, including siblings.
  if (source.direction !== target.direction) return { kind: "invalid", reason: "same-direction" };
  const moved = model.pinConnections[sourceId] ?? [];
  if (
    !source.connections.canMove ||
    !moved.length ||
    (!target.connections.canAppend && !target.connections.canReplace) ||
    (target.connections.maximum !== null &&
      moved.length + (target.connections.canReplace ? 0 : target.connections.current) >
        target.connections.maximum)
  ) {
    return { kind: "invalid", reason: "capacity" };
  }
  for (const id of moved) {
    const connection = model.connections[id];
    const other = model.pins[connection?.from === sourceId ? connection.to : connection?.from];
    if (!other) return { kind: "invalid", reason: "orphan" };
    // The peer keeps its existing link, so its occupied capacity is not a new append.
    const feedback = resolveConnectionCompatibility(target, {
      ...other,
      connections: { ...other.connections, canAppend: true, canReplace: false },
    });
    if (feedback.kind === "invalid") return { kind: "invalid", reason: REASONS[feedback.reason] };
  }
  return target.connections.canReplace
    ? { kind: "replace", displacedConnectionIds: model.pinConnections[targetId] ?? [] }
    : { kind: "append" };
}

export function resolveFlowPinAction(
  event: Pick<MouseEvent, "button" | "altKey" | "ctrlKey" | "metaKey">,
  pin: PinData,
): "none" | "disconnect" | FlowConnectionIntent {
  if (event.button !== 0 || pin.orphan) return "none";
  if (event.altKey) return pin.connections.canMove ? "disconnect" : "none";
  if (event.ctrlKey || event.metaKey) {
    return pin.connections.canMove && pin.connections.current > 0 ? "moveConnections" : "none";
  }
  return pin.connections.canAppend || pin.connections.canReplace ? "connect" : "none";
}
