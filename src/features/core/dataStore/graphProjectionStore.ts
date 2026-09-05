import { create } from "zustand";
import type {
  ConnectionData,
  ConnectionId,
  GraphPath,
  NodeData,
  NodeId,
  PinData,
  PinId,
} from "@/features/domain/editorProjection/graphRuntimeTypes";
import type {
  EditorGraphProjectionDto,
  GraphProjectionReplacementDto,
} from "@/shared/types/domain/editorProjection";
import { portAddressKey, toProjectionEntities } from "@/features/domain/editorProjection";
import type { EditorProjectionEntities } from "@/features/domain/editorProjection";
import {
  type GraphEntityBucket,
  getGraphConnection,
  getGraphConnections,
  getGraphNode,
  getGraphNodeIds,
  getGraphNodePins,
  getGraphPin,
  getGraphPinConnections,
  hasGraphData,
} from "./graphEntityAccess";

export type { GraphEntityBucket } from "./graphEntityAccess";

export type ProjectionApplyResult =
  | { applied: true }
  | { applied: false; reason: "invalid"; error: unknown };

export interface PreparedGraphProjectionReplacements {
  readonly graphPaths: readonly string[];
  readonly graphEntities: Readonly<Record<GraphPath, GraphEntityBucket>>;
}

export type ProjectionPreparationResult =
  | { prepared: true; plan: PreparedGraphProjectionReplacements }
  | {
      prepared: false;
      reason: "duplicate-graph-path" | "invalid";
      graphPath: string;
      error?: unknown;
    };

export type AtomicProjectionApplyResult =
  | { applied: true; graphPaths: string[] }
  | {
      applied: false;
      reason: "duplicate-graph-path" | "invalid";
      graphPath: string;
      error?: unknown;
    };

function buildProjectionBucket(entities: EditorProjectionEntities): GraphEntityBucket {
  const bucket: GraphEntityBucket = {
    basis: entities.basis,
    diagnostics: entities.diagnostics,
    outcome: entities.outcome,
    hasBlockingDiagnostics: entities.hasBlockingDiagnostics,
    nodes: {},
    pins: {},
    connections: {},
    graphNodes: [],
    pinConnections: {},
  };

  for (const node of Object.values(entities.nodes)) {
    const portIds = entities.portIdsByNodeId[node.nodeId];
    bucket.nodes[node.nodeId] = {
      id: node.nodeId,
      graphPath: node.graphPath,
      nodeType: node.nodeTypeId,
      position: node.position,
      pinIds: portIds,
      display: node.display,
      parameterEditors: node.parameterEditors,
      portInstanceAdditions: node.portInstanceAdditions,
      capabilities: node.capabilities,
      diagnostics: node.diagnostics,
    };
    bucket.graphNodes.push(node.nodeId);
  }

  for (const [portId, port] of Object.entries(entities.ports)) {
    bucket.pins[portId] = {
      id: portId,
      nodeId: port.address.nodeId,
      name: port.display.instanceLabel ?? port.display.label,
      direction: port.direction,
      address: port.address,
      display: port.display,
      orphan: port.orphan,
      canRemove: port.canRemove,
      connections: port.connections,
      input: port.input,
      acceptedType: port.acceptedType,
      typeState: port.typeState,
      resolvedSchema: port.resolvedSchema,
      status: port.status,
    };
    bucket.pinConnections[portId] = entities.connectionIdsByPortId[portId];
  }

  for (const connection of Object.values(entities.connections)) {
    bucket.connections[connection.connectionId] = {
      id: connection.connectionId,
      from: portAddressKey(connection.output),
      to: portAddressKey(connection.input),
      output: connection.output,
      input: connection.input,
      order: connection.order,
    };
  }

  return bucket;
}

function buildProjectionCandidate(
  graphPath: string,
  projection: EditorGraphProjectionDto,
): GraphEntityBucket {
  const entities = toProjectionEntities(projection);
  if (entities.graphPath !== graphPath) {
    throw new Error(
      `projection graph path '${entities.graphPath}' does not match requested graph path '${graphPath}'`,
    );
  }
  return buildProjectionBucket(entities);
}

export function prepareGraphProjectionReplacements(
  replacements: readonly GraphProjectionReplacementDto[],
  baseGraphEntities: Readonly<
    Record<GraphPath, GraphEntityBucket>
  > = useGraphProjectionStore.getState().graphEntities,
): ProjectionPreparationResult {
  const graphPaths = replacements.map(({ graphPath }) => graphPath);
  const seen = new Set<string>();
  const candidates: Array<[string, GraphEntityBucket]> = [];
  for (const replacement of replacements) {
    if (seen.has(replacement.graphPath)) {
      return { prepared: false, reason: "duplicate-graph-path", graphPath: replacement.graphPath };
    }
    seen.add(replacement.graphPath);
    try {
      candidates.push([
        replacement.graphPath,
        buildProjectionCandidate(replacement.graphPath, replacement.projection),
      ]);
    } catch (error) {
      return { prepared: false, reason: "invalid", graphPath: replacement.graphPath, error };
    }
  }
  return {
    prepared: true,
    plan: {
      graphPaths,
      graphEntities: {
        ...baseGraphEntities,
        ...Object.fromEntries(candidates),
      },
    },
  };
}

export function commitPreparedGraphProjectionReplacements(
  plan: PreparedGraphProjectionReplacements,
): void {
  useGraphProjectionStore.setState((state) => ({
    graphEntities: {
      ...state.graphEntities,
      ...Object.fromEntries(
        plan.graphPaths.map((graphPath) => [graphPath, plan.graphEntities[graphPath]]),
      ),
    },
  }));
}

interface GraphProjectionStore {
  graphEntities: Record<GraphPath, GraphEntityBucket>;
  getGraphNode(graphPath: GraphPath, nodeId: NodeId): NodeData | undefined;
  getGraphPin(graphPath: GraphPath, pinId: PinId): PinData | undefined;
  getGraphNodeIds(graphPath: GraphPath): NodeId[];
  getGraphNodePins(graphPath: GraphPath, nodeId: NodeId): PinId[];
  getGraphPinConnections(graphPath: GraphPath, pinId: PinId): ConnectionId[];
  getGraphConnection(graphPath: GraphPath, connectionId: ConnectionId): ConnectionData | undefined;
  getGraphConnections(graphPath: GraphPath): ConnectionData[];
  hasGraph(graphPath: GraphPath): boolean;
  clearGraph(graphPath: GraphPath): void;
  replaceProjection(
    graphPath: GraphPath,
    projection: EditorGraphProjectionDto,
  ): ProjectionApplyResult;
  replaceProjectionsAtomically(
    replacements: GraphProjectionReplacementDto[],
  ): AtomicProjectionApplyResult;
}

export const useGraphProjectionStore = create<GraphProjectionStore>((set, get) => ({
  graphEntities: {},

  getGraphNode: (graphPath, nodeId) => getGraphNode(get(), graphPath, nodeId),
  getGraphPin: (graphPath, pinId) => getGraphPin(get(), graphPath, pinId),
  getGraphNodeIds: (graphPath) => getGraphNodeIds(get(), graphPath),
  getGraphNodePins: (graphPath, nodeId) => getGraphNodePins(get(), graphPath, nodeId),
  getGraphPinConnections: (graphPath, pinId) => getGraphPinConnections(get(), graphPath, pinId),
  getGraphConnection: (graphPath, connectionId) =>
    getGraphConnection(get(), graphPath, connectionId),
  getGraphConnections: (graphPath) => getGraphConnections(get(), graphPath),
  hasGraph: (graphPath) => hasGraphData(get(), graphPath),

  clearGraph: (graphPath) =>
    set((state) => {
      if (!state.graphEntities[graphPath]) return state;
      const graphEntities = { ...state.graphEntities };
      delete graphEntities[graphPath];
      return { graphEntities };
    }),

  replaceProjection: (graphPath, projection) => {
    let candidate: GraphEntityBucket;
    try {
      candidate = buildProjectionCandidate(graphPath, projection);
    } catch (error) {
      return { applied: false, reason: "invalid", error };
    }
    set((state) => ({
      graphEntities: { ...state.graphEntities, [graphPath]: candidate },
    }));
    return { applied: true };
  },

  replaceProjectionsAtomically: (replacements) => {
    const prepared = prepareGraphProjectionReplacements(replacements, get().graphEntities);
    if (!prepared.prepared) return { applied: false, ...prepared };
    commitPreparedGraphProjectionReplacements(prepared.plan);
    return { applied: true, graphPaths: [...prepared.plan.graphPaths] };
  },
}));
