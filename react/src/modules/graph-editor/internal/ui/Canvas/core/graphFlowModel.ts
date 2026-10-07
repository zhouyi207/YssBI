import type { Node, Edge } from "@xyflow/react";
import type { GraphProjectionSnapshot } from "@/features/core/graph/read";
import type { PinData } from "@/features/domain/editorProjection/graphRuntimeTypes";
import type { DeepReadonly } from "@/shared/types/deepReadonly";
import { pinTypeColorKey } from "@/shared/types/domain/pinSemantics";
import type {
  ConnectionDecision,
  ConnectionIntent,
} from "@/shared/types/domain/connectionCandidates";

export type GraphFlowNode = Node<{ handlesKey: string }, "graph">;
export type GraphFlowEdge = Edge<{ fromPinId: string; toPinId: string }, "graph">;
export type FlowConnectionFeedback = ConnectionDecision | { kind: "pending" };

export interface GraphFlowModel {
  nodes: GraphFlowNode[];
  nodeIds: ReadonlySet<string>;
  edges: GraphFlowEdge[];
  pins: DeepReadonly<Record<string, PinData>>;
  draggableNodeIds: ReadonlySet<string>;
}

interface GraphFlowEdgeAppearance {
  colorKey: string | undefined;
  hasEndpoints: boolean;
  blocked: boolean;
}

/** One edge's read selector; unrelated pin fields do not invalidate its appearance. */
export function createGraphFlowEdgeAppearanceSelector(
  graphPath: string,
  connectionId: string,
  fromPinId: string | undefined,
  toPinId: string | undefined,
) {
  let previousFrom: DeepReadonly<PinData> | undefined;
  let previousTo: DeepReadonly<PinData> | undefined;
  let view: GraphFlowEdgeAppearance = {
    colorKey: undefined,
    hasEndpoints: false,
    blocked: false,
  };
  return (snapshot: GraphProjectionSnapshot): GraphFlowEdgeAppearance => {
    const graph = snapshot.graphEntities[graphPath];
    const from = fromPinId === undefined ? undefined : graph?.pins[fromPinId];
    const to = toPinId === undefined ? undefined : graph?.pins[toPinId];
    const blocked =
      graph?.blockedConnectionIds[connectionId] === true ||
      from?.orphan === true ||
      to?.orphan === true;
    if (from === previousFrom && to === previousTo && blocked === view.blocked) return view;
    const colorKey = from ? pinTypeColorKey(from) : undefined;
    const hasEndpoints = Boolean(from && to);
    if (
      colorKey !== view.colorKey ||
      hasEndpoints !== view.hasEndpoints ||
      blocked !== view.blocked
    ) {
      view = { colorKey, hasEndpoints, blocked };
    }
    previousFrom = from;
    previousTo = to;
    return view;
  };
}

/** One panel's edge selection overlay, indexed in the same cache as its rendered views. */
export function createGraphFlowEdgeViewProjector() {
  const cache = new Map<string, { index: number; view: GraphFlowEdge }>();
  let previousProjections: GraphFlowEdge[] | undefined;
  let previousSelected: ReadonlySet<string> = new Set();
  let previousInteractive = false;
  let edges: GraphFlowEdge[] = [];

  return (
    projections: GraphFlowEdge[],
    selectedIds: ReadonlySet<string>,
    interactive: boolean,
  ): GraphFlowEdge[] => {
    const sameBasis = previousProjections === projections && previousInteractive === interactive;
    if (sameBasis && (!interactive || previousSelected === selectedIds)) {
      previousSelected = selectedIds;
      return edges;
    }
    let next = edges;
    // For dense selections, one sequential pass is cheaper than visiting both selection sets.
    if (sameBasis && selectedIds.size + previousSelected.size < projections.length) {
      const updateSelected = (id: string, selected: boolean) => {
        const entry = cache.get(id);
        if (!entry || entry.view.selected === selected) return;
        if (next === edges) next = edges.slice();
        entry.view = { ...entry.view, selected };
        next[entry.index] = entry.view;
      };
      for (const id of selectedIds) {
        if (!previousSelected.has(id)) updateSelected(id, true);
      }
      for (const id of previousSelected) {
        if (!selectedIds.has(id)) updateSelected(id, false);
      }
    } else {
      let changed = projections.length !== edges.length;
      next = projections.map((projection, index) => {
        const selected = interactive && selectedIds.has(projection.id);
        const entry = previousProjections === undefined ? undefined : cache.get(projection.id);
        const before = entry ? previousProjections?.[entry.index] : undefined;
        let view = entry?.view;
        if (before !== projection || !view || view.selected !== selected) {
          view = { ...projection, selected };
          if (entry) {
            entry.view = view;
          } else {
            cache.set(projection.id, { index, view });
          }
        }
        if (entry) entry.index = index;
        if (!changed && view !== edges[index]) changed = true;
        return view;
      });
      if (!changed) next = edges;
      if (cache.size !== projections.length) {
        for (const [id, entry] of cache) {
          if (projections[entry.index]?.id !== id) cache.delete(id);
        }
      }
    }
    previousProjections = projections;
    previousSelected = selectedIds;
    previousInteractive = interactive;
    edges = next;
    return edges;
  };
}

export interface FlowInteractionProjection {
  sourceId: string | null;
  pins: GraphFlowModel["pins"];
  decisions: Readonly<Partial<Record<string, ConnectionDecision>>>;
  dimmedNodes: ReadonlySet<string>;
}

const EMPTY_PINS: GraphFlowModel["pins"] = {};
const PENDING_FEEDBACK: FlowConnectionFeedback = Object.freeze({ kind: "pending" });

export const EMPTY_FLOW_INTERACTION: FlowInteractionProjection = {
  sourceId: null,
  pins: EMPTY_PINS,
  decisions: {},
  dimmedNodes: new Set(),
};

export function getFlowPinFeedback(
  state: FlowInteractionProjection,
  pinId: string,
): FlowConnectionFeedback | null {
  if (state.sourceId === null || !state.pins[pinId]) return null;
  return state.decisions[pinId] ?? PENDING_FEEDBACK;
}

/** A primitive selector result keeps unrelated decision changes out of pin rendering. */
export function getFlowPinAppearance(
  state: FlowInteractionProjection,
  pinId: string,
): "active" | "highlighted" | "dimmed" | "neutral" {
  const feedback = getFlowPinFeedback(state, pinId);
  if (!feedback) return "neutral";
  if (state.sourceId === pinId) return "active";
  if (feedback.kind === "pending") return "neutral";
  return feedback.kind === "invalid" ? "dimmed" : "highlighted";
}

/** Panel-local decoration facts, separate from committed node content. */
export function projectFlowInteraction(
  pins: GraphFlowModel["pins"],
  nodeIds: ReadonlySet<string>,
  sourceId: string | undefined,
  decisions: FlowInteractionProjection["decisions"],
): FlowInteractionProjection {
  const source = sourceId ? pins[sourceId] : undefined;
  if (!source) return EMPTY_FLOW_INTERACTION;
  const dimmedNodes = new Set(nodeIds);
  dimmedNodes.delete(source.nodeId);
  for (const id in pins) {
    const pin = pins[id];
    if (decisions[pin.id]?.kind !== "invalid") dimmedNodes.delete(pin.nodeId);
  }
  return {
    sourceId: source.id,
    pins,
    decisions,
    dimmedNodes,
  };
}

function reuseArray<T>(previous: T[], next: T[]): T[] {
  return previous.length === next.length && next.every((value, index) => value === previous[index])
    ? previous
    : next;
}

type GraphBucket = GraphProjectionSnapshot["graphEntities"][string] | undefined;

/** One panel's render cache. Inputs are immutable buckets published by the Graph owner. */
export function createGraphFlowModelProjector() {
  let previous: GraphBucket;
  let model: GraphFlowModel = {
    nodes: [],
    nodeIds: new Set(),
    edges: [],
    pins: EMPTY_PINS,
    draggableNodeIds: new Set(),
  };
  const nodesById = new Map<string, GraphFlowNode>();
  const edgesById = new Map<string, GraphFlowEdge>();

  return (bucket: GraphBucket): GraphFlowModel => {
    if (bucket === previous) return model;
    let { nodes, nodeIds, edges, draggableNodeIds } = model;
    const pins = bucket?.pins ?? EMPTY_PINS;

    if (bucket?.nodes !== previous?.nodes || bucket?.graphNodes !== previous?.graphNodes) {
      const next: GraphFlowNode[] = [];
      for (const id of bucket?.graphNodes ?? []) {
        const node = bucket?.nodes[id];
        if (!node) continue;
        const old = nodesById.get(id);
        if (old && node === previous?.nodes[id]) {
          next.push(old);
          continue;
        }
        const handlesKey =
          old && node.pinIds === previous?.nodes[id]?.pinIds
            ? old.data.handlesKey
            : node.pinIds.join("\u0000");
        const draggable = !node.capabilities.managed;
        const view: GraphFlowNode =
          old &&
          old.position.x === node.position.x &&
          old.position.y === node.position.y &&
          old.data.handlesKey === handlesKey &&
          old.draggable === draggable
            ? old
            : {
                id,
                type: "graph",
                position: { ...node.position },
                data: old?.data.handlesKey === handlesKey ? old.data : { handlesKey },
                selectable: draggable,
                draggable,
                deletable: false,
              };
        if (view !== old) nodesById.set(id, view);
        next.push(view);
      }
      nodes = reuseArray(nodes, next);
      if (nodes !== model.nodes) {
        if (nodes.length !== nodeIds.size || nodes.some((node) => !nodeIds.has(node.id))) {
          nodeIds = new Set(nodes.map((node) => node.id));
          for (const id of nodesById.keys()) {
            if (!nodeIds.has(id)) nodesById.delete(id);
          }
        }
        let draggableCount = 0;
        let sameDraggableIds = true;
        for (const node of nodes) {
          if (!node.draggable) continue;
          draggableCount++;
          if (!draggableNodeIds.has(node.id)) sameDraggableIds = false;
        }
        if (!sameDraggableIds || draggableCount !== draggableNodeIds.size) {
          const ids = new Set<string>();
          for (const node of nodes) {
            if (node.draggable) ids.add(node.id);
          }
          draggableNodeIds = ids;
        }
      }
    }

    // The graph owner validates endpoints before publishing this bucket. Read topology
    // from its connection records so pin values, labels and types do not rescan edges.
    if (bucket?.connections !== previous?.connections || nodeIds !== model.nodeIds) {
      const next: GraphFlowEdge[] = [];
      const connections = bucket?.connections;
      for (const id in connections) {
        const connection = connections[id];
        const source = connection.output.nodeId;
        const target = connection.input.nodeId;
        if (!nodeIds.has(source) || !nodeIds.has(target)) {
          edgesById.delete(id);
          continue;
        }
        const old = edgesById.get(id);
        const view: GraphFlowEdge =
          old &&
          old.source === source &&
          old.target === target &&
          old.sourceHandle === connection.from &&
          old.targetHandle === connection.to
            ? old
            : {
                id: connection.id,
                type: "graph",
                source,
                target,
                sourceHandle: connection.from,
                targetHandle: connection.to,
                data: { fromPinId: connection.from, toPinId: connection.to },
                deletable: false,
                reconnectable: false,
                selectable: false,
              };
        if (view !== old) edgesById.set(id, view);
        next.push(view);
      }
      edges = reuseArray(edges, next);
      if (edgesById.size !== edges.length) {
        for (const id of edgesById.keys()) {
          if (!connections?.[id]) edgesById.delete(id);
        }
      }
    }

    previous = bucket;
    if (nodes !== model.nodes || edges !== model.edges || pins !== model.pins)
      model = { nodes, nodeIds, edges, pins, draggableNodeIds };
    return model;
  };
}

export function resolveFlowPinAction(
  event: Pick<MouseEvent, "button" | "altKey" | "ctrlKey" | "metaKey">,
  pin: DeepReadonly<PinData>,
): "none" | "disconnect" | ConnectionIntent {
  if (event.button !== 0 || pin.orphan) return "none";
  if (event.altKey) return pin.connections.canMove ? "disconnect" : "none";
  if (event.ctrlKey || event.metaKey) {
    return pin.connections.canMove ? "moveConnections" : "none";
  }
  return pin.connections.canAppend || pin.connections.canReplace ? "connect" : "none";
}
