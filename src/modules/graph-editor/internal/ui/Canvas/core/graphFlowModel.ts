import type { Node, Edge } from "@xyflow/react";
import type { GraphProjectionSnapshot } from "@/features/core/graph/read";
import type { PinData, ConnectionData } from "@/features/domain/editorProjection/graphRuntimeTypes";
import type { DeepReadonly } from "@/shared/types/deepReadonly";
import { resolveConnectionCompatibility } from "@/features/domain/editorProjection/connectionRules";
import { shareProjection } from "@/features/core/state/readProjection";

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

export interface GraphFlowModel {
  nodes: GraphFlowNode[];
  nodeIds: ReadonlySet<string>;
  edges: GraphFlowEdge[];
  pins: DeepReadonly<Record<string, PinData>>;
  connections: DeepReadonly<Record<string, ConnectionData>>;
  pinConnections: Readonly<Record<string, readonly string[]>>;
  draggableNodeIds: ReadonlySet<string>;
}

export interface FlowPinInteraction {
  active: boolean;
  dragState: "highlighted" | "dimmed";
  feedback: FlowConnectionFeedback;
}

export interface FlowInteractionProjection {
  sourceId: string | null;
  sourceDirection: PinData["direction"] | null;
  pins: Readonly<Record<string, FlowPinInteraction>>;
  dimmedNodes: ReadonlySet<string>;
}

export const EMPTY_FLOW_INTERACTION: FlowInteractionProjection = {
  sourceId: null,
  sourceDirection: null,
  pins: {},
  dimmedNodes: new Set(),
};

/** Panel-local decoration facts, separate from committed node content. */
export function projectFlowInteraction(
  model: GraphFlowModel,
  sourceId: string | undefined,
  intent: FlowConnectionIntent,
  previous: FlowInteractionProjection = EMPTY_FLOW_INTERACTION,
): FlowInteractionProjection {
  const source = sourceId ? model.pins[sourceId] : undefined;
  if (!source) return EMPTY_FLOW_INTERACTION;
  const pins: Record<string, FlowPinInteraction> = {};
  const compatibleNodes = new Set<string>();
  for (const pin of Object.values(model.pins)) {
    const feedback = resolveFlowConnection(model, source.id, pin.id, intent);
    const active = pin.id === source.id;
    if (feedback.kind !== "invalid") compatibleNodes.add(pin.nodeId);
    pins[pin.id] = shareProjection(previous.pins[pin.id], {
      active,
      feedback,
      dragState: active || feedback.kind !== "invalid" ? "highlighted" : "dimmed",
    });
  }
  return {
    sourceId: source.id,
    sourceDirection: source.direction,
    pins,
    dimmedNodes: new Set(
      [...model.nodeIds].filter((id) => id !== source.nodeId && !compatibleNodes.has(id)),
    ),
  };
}

function reuseArray<T>(previous: T[] | undefined, next: T[]): T[] {
  return previous?.length === next.length && next.every((value, index) => value === previous[index])
    ? previous
    : next;
}

export function buildGraphFlowModel(
  bucket: GraphProjectionSnapshot["graphEntities"][string] | undefined,
  previous?: GraphFlowModel,
): GraphFlowModel {
  const pins = bucket?.pins ?? {};
  const previousNodes = new Map(previous?.nodes.map((node) => [node.id, node]));
  const nodes = reuseArray(
    previous?.nodes,
    (bucket?.graphNodes ?? []).flatMap((id) => {
      const node = bucket?.nodes[id];
      if (!node) return [];
      const old = previousNodes.get(id);
      const handlesKey = node.pinIds.join("\u0000");
      const draggable = !node.capabilities.managed;
      if (
        old &&
        old.position.x === node.position.x &&
        old.position.y === node.position.y &&
        old.data.handlesKey === handlesKey &&
        old.draggable === draggable
      )
        return [old];
      return [
        {
          id,
          type: "graph" as const,
          position: { ...node.position },
          data: old?.data.handlesKey === handlesKey ? old.data : { handlesKey },
          selectable: draggable,
          draggable,
          deletable: false,
        },
      ];
    }),
  );
  const nodeIds =
    previous &&
    nodes.length === previous.nodeIds.size &&
    nodes.every((node) => previous.nodeIds.has(node.id))
      ? previous.nodeIds
      : new Set(nodes.map((node) => node.id));
  const connections = bucket?.connections ?? {};
  const previousEdges = new Map(previous?.edges.map((edge) => [edge.id, edge]));
  const edges = reuseArray(
    previous?.edges,
    Object.values(connections).flatMap((connection) => {
      const from = pins[connection.from],
        to = pins[connection.to];
      if (!from || !to || !nodeIds.has(from.nodeId) || !nodeIds.has(to.nodeId)) return [];
      const old = previousEdges.get(connection.id);
      if (
        old &&
        old.source === from.nodeId &&
        old.target === to.nodeId &&
        old.sourceHandle === from.id &&
        old.targetHandle === to.id
      )
        return [old];
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
    }),
  );
  const draggableIds = nodes.filter((node) => node.draggable).map((node) => node.id);
  return {
    nodes,
    nodeIds,
    edges,
    pins,
    connections,
    pinConnections: bucket?.pinConnections ?? {},
    draggableNodeIds:
      previous &&
      draggableIds.length === previous.draggableNodeIds.size &&
      draggableIds.every((id) => previous.draggableNodeIds.has(id))
        ? previous.draggableNodeIds
        : new Set(draggableIds),
  };
}

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
  pin: DeepReadonly<PinData>,
): "none" | "disconnect" | FlowConnectionIntent {
  if (event.button !== 0 || pin.orphan) return "none";
  if (event.altKey) return pin.connections.canMove ? "disconnect" : "none";
  if (event.ctrlKey || event.metaKey) {
    return pin.connections.canMove && pin.connections.current > 0 ? "moveConnections" : "none";
  }
  return pin.connections.canAppend || pin.connections.canReplace ? "connect" : "none";
}
