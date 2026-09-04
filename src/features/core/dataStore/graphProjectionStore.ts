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
  | { applied: true; reason: "newer" }
  | { applied: false; reason: "invalid"; error: unknown }
  | { applied: false; reason: "stale-generation" | "older-revision" };

export interface PreparedGraphProjectionReplacements {
  readonly graphPaths: readonly string[];
  readonly graphEntities: Readonly<Record<GraphPath, GraphEntityBucket>>;
}

export type ProjectionPreparationResult =
  | { prepared: true; plan: PreparedGraphProjectionReplacements }
  | {
      prepared: false;
      reason: "duplicate-graph-path" | "invalid" | "non-monotonic-revision";
      graphPath: string;
      error?: unknown;
    };

export type AtomicProjectionApplyResult =
  | { applied: true; graphPaths: string[] }
  | {
      applied: false;
      reason: "duplicate-graph-path" | "invalid" | "non-monotonic-revision";
      graphPath: string;
      error?: unknown;
    };

function commitGraphBucket(
  state: { graphEntities: Record<GraphPath, GraphEntityBucket> },
  graphPath: GraphPath,
  bucket: GraphEntityBucket,
) {
  return { graphEntities: { ...state.graphEntities, [graphPath]: bucket } };
}

function buildProjectionBucket(
  entities: EditorProjectionEntities,
  requestGeneration: number,
): GraphEntityBucket {
  const bucket: GraphEntityBucket = {
    basis: entities.basis,
    sourceRevision: entities.sourceRevision,
    requestGeneration,
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
      dataType: port.resolvedType?.resolved ? (port.resolvedType.dataType ?? undefined) : undefined,
      address: port.address,
      display: port.display,
      orphan: port.orphan,
      canRemove: port.canRemove,
      connections: port.connections,
      input: port.input,
      resolvedType: port.resolvedType,
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
  requestGeneration: number,
): GraphEntityBucket {
  const entities = toProjectionEntities(projection);
  if (entities.graphPath !== graphPath) {
    throw new Error(
      `projection graph path '${entities.graphPath}' does not match requested graph path '${graphPath}'`,
    );
  }
  return buildProjectionBucket(entities, requestGeneration);
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
    const current = baseGraphEntities[replacement.graphPath];
    let candidate: GraphEntityBucket;
    try {
      candidate = buildProjectionCandidate(
        replacement.graphPath,
        replacement.projection,
        (current?.requestGeneration ?? 0) + 1,
      );
    } catch (error) {
      return { prepared: false, reason: "invalid", graphPath: replacement.graphPath, error };
    }
    if (current && candidate.sourceRevision < current.sourceRevision) {
      return {
        prepared: false,
        reason: "non-monotonic-revision",
        graphPath: replacement.graphPath,
      };
    }
    candidates.push([replacement.graphPath, candidate]);
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
  useGraphProjectionStore.setState({ graphEntities: { ...plan.graphEntities } });
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
    requestGeneration: number,
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

  replaceProjection: (graphPath, projection, requestGeneration) => {
    let candidate: GraphEntityBucket;
    try {
      candidate = buildProjectionCandidate(graphPath, projection, requestGeneration);
    } catch (error) {
      return { applied: false, reason: "invalid", error };
    }

    let result: ProjectionApplyResult = { applied: false, reason: "stale-generation" };
    set((state) => {
      const current = state.graphEntities[graphPath];
      if (current && requestGeneration <= current.requestGeneration) return state;
      if (current && candidate.sourceRevision < current.sourceRevision) {
        result = { applied: false, reason: "older-revision" };
        return state;
      }
      result = { applied: true, reason: "newer" };
      return commitGraphBucket(state, graphPath, candidate);
    });
    return result;
  },

  replaceProjectionsAtomically: (replacements) => {
    const prepared = prepareGraphProjectionReplacements(replacements, get().graphEntities);
    if (!prepared.prepared) return { applied: false, ...prepared };
    commitPreparedGraphProjectionReplacements(prepared.plan);
    return { applied: true, graphPaths: [...prepared.plan.graphPaths] };
  },
}));
