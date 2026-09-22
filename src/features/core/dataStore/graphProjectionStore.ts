import { freezePublishedValue } from "@/shared/types/deepReadonly";
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
  GraphDocumentDto,
  GraphEditorSessionDto,
  GraphEditVersionDto,
} from "@/shared/types/domain/editorMutation";
import type { GraphResultState } from "@/shared/types/domain/result";
import { portAddressKey, toProjectionEntities } from "@/features/domain/editorProjection";
import { shareProjection } from "@/features/core/state/readProjection";
import {
  type GraphEntityBucket,
  getGraphConnection,
  getGraphNode,
  getGraphNodeIds,
  getGraphNodePins,
  getGraphPin,
  getGraphPinConnections,
  hasGraphData,
} from "./graphEntityAccess";

export type { GraphEntityBucket } from "./graphEntityAccess";

/** A read mirror of the Rust editing session; the backend owns document/history writes. */
export interface GraphEditorState {
  readonly document: GraphDocumentDto;
  readonly projection: EditorGraphProjectionDto;
  readonly version: GraphEditVersionDto;
  readonly sessionId: number;
  readonly projectionGeneration: number;
  readonly semanticInputHash: string;
  readonly saveDirty: boolean;
  readonly saving: boolean;
  readonly canUndo: boolean;
  readonly canRedo: boolean;
}

export interface GraphProjectionData {
  graphEntities: Record<GraphPath, GraphEntityBucket>;
  sessions: Readonly<Record<GraphPath, GraphEditorState>>;
  resultStates: Readonly<Record<GraphPath, GraphResultState>>;
}

export function canAcceptGraphSession(
  previous: GraphEditorState | undefined,
  input: GraphEditorSessionDto,
): boolean {
  return (
    previous?.version.sessionId !== input.editing.version.sessionId ||
    BigInt(previous.version.revision) <= BigInt(input.editing.version.revision)
  );
}

function sameFields<T extends object>(previous: T | undefined, next: T): previous is T {
  if (!previous) return false;
  const keys = Object.keys(next) as Array<keyof T>;
  return (
    keys.length === Object.keys(previous).length &&
    keys.every(
      (key) =>
        Object.prototype.hasOwnProperty.call(previous, key) && Object.is(previous[key], next[key]),
    )
  );
}

function shareEntity<T extends object>(previous: T | undefined, next: T): T {
  return sameFields(previous, next) ? previous : shareProjection(previous, next);
}

function shareIds(previous: string[] | undefined, next: string[]): string[] {
  return previous?.length === next.length && next.every((id, i) => id === previous[i])
    ? previous
    : next;
}

function buildProjectionBucket(
  graphPath: string,
  projection: EditorGraphProjectionDto,
  previous?: GraphEntityBucket,
): GraphEntityBucket {
  const entities = toProjectionEntities(projection);
  if (entities.graphPath !== graphPath)
    throw new Error(`Projection '${entities.graphPath}' does not match '${graphPath}'`);
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
    bucket.nodes[node.nodeId] = shareEntity(previous?.nodes[node.nodeId], {
      id: node.nodeId,
      graphPath: node.graphPath,
      nodeType: node.nodeTypeId,
      position: node.position,
      pinIds: shareIds(previous?.nodes[node.nodeId]?.pinIds, entities.portIdsByNodeId[node.nodeId]),
      display: node.display,
      parameterEditors: node.parameterEditors,
      portInstanceAdditions: node.portInstanceAdditions,
      capabilities: node.capabilities,
      diagnostics: node.diagnostics,
    });
    bucket.graphNodes.push(node.nodeId);
  }
  for (const [portId, port] of Object.entries(entities.ports)) {
    bucket.pins[portId] = shareEntity(previous?.pins[portId], {
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
    });
    bucket.pinConnections[portId] = shareIds(
      previous?.pinConnections[portId],
      entities.connectionIdsByPortId[portId],
    );
  }
  for (const connection of Object.values(entities.connections)) {
    const existing = previous?.connections[connection.connectionId];
    const from = portAddressKey(connection.output),
      to = portAddressKey(connection.input);
    bucket.connections[connection.connectionId] = shareEntity(existing, {
      id: connection.connectionId,
      from,
      to,
      output: existing?.from === from ? existing.output : connection.output,
      input: existing?.to === to ? existing.input : connection.input,
      order: connection.order,
    });
  }
  bucket.graphNodes = shareIds(previous?.graphNodes, bucket.graphNodes);
  if (sameFields(previous?.nodes, bucket.nodes)) bucket.nodes = previous.nodes;
  if (sameFields(previous?.pins, bucket.pins)) bucket.pins = previous.pins;
  if (sameFields(previous?.connections, bucket.connections))
    bucket.connections = previous.connections;
  if (sameFields(previous?.pinConnections, bucket.pinConnections))
    bucket.pinConnections = previous.pinConnections;
  return sameFields(previous, bucket) ? previous : bucket;
}

let nextViewSessionId = 0;

function prepareSession(
  state: GraphProjectionData,
  graphPath: string,
  input: GraphEditorSessionDto,
  saving?: boolean,
  renew = false,
): GraphProjectionData {
  const previous = state.sessions[graphPath];
  if (!canAcceptGraphSession(previous, input)) return state;
  if (input.resultState.semanticInputHash !== input.projection.basis.semanticInputHash) {
    throw new Error("Graph projection and result state must have the same semantic identity");
  }
  const document = shareProjection(previous?.document, input.document);
  const projection = shareProjection(previous?.projection, input.projection);
  const currentResults = state.resultStates[graphPath];
  const resultState =
    currentResults &&
    currentResults.executionSessionId === input.resultState.executionSessionId &&
    currentResults.semanticInputHash === input.resultState.semanticInputHash &&
    BigInt(currentResults.revision) > BigInt(input.resultState.revision)
      ? currentResults
      : shareProjection(currentResults, input.resultState);
  const sessionUnchanged =
    !renew &&
    previous &&
    previous.document === document &&
    previous.projection === projection &&
    previous.version.sessionId === input.editing.version.sessionId &&
    previous.version.revision === input.editing.version.revision &&
    previous.saveDirty === input.editing.dirty &&
    previous.canUndo === input.editing.canUndo &&
    previous.canRedo === input.editing.canRedo &&
    (saving === undefined || saving === previous.saving);
  if (sessionUnchanged && resultState === state.resultStates[graphPath]) return state;
  const bucket =
    projection === previous?.projection && state.graphEntities[graphPath]
      ? state.graphEntities[graphPath]
      : buildProjectionBucket(graphPath, projection, state.graphEntities[graphPath]);
  const next: GraphProjectionData = {
    sessions: sessionUnchanged
      ? state.sessions
      : {
          ...state.sessions,
          [graphPath]: {
            document,
            projection,
            version: shareProjection(previous?.version, input.editing.version),
            sessionId:
              !renew && previous?.version.sessionId === input.editing.version.sessionId
                ? previous.sessionId
                : ++nextViewSessionId,
            projectionGeneration: (previous?.projectionGeneration ?? 0) + 1,
            semanticInputHash: projection.basis.semanticInputHash,
            saveDirty: input.editing.dirty,
            canUndo: input.editing.canUndo,
            canRedo: input.editing.canRedo,
            saving: saving ?? previous?.saving ?? false,
          },
        },
    graphEntities:
      bucket === state.graphEntities[graphPath]
        ? state.graphEntities
        : { ...state.graphEntities, [graphPath]: bucket },
    resultStates:
      resultState === state.resultStates[graphPath]
        ? state.resultStates
        : { ...state.resultStates, [graphPath]: resultState },
  };
  freezePublishedValue(next);
  return next;
}

export interface PreparedGraphSessions {
  readonly graphPaths: readonly string[];
  readonly state: GraphProjectionData;
}

/** Prepare all accepted sessions before a single publication, including project snapshots. */
export function prepareGraphSessions(
  replacements: readonly { graphPath: string; session: GraphEditorSessionDto }[],
  retainedGraphPaths?: ReadonlySet<string>,
  base: GraphProjectionData = useGraphProjectionStore.getState(),
): PreparedGraphSessions {
  const retain = <T>(values: Readonly<Record<string, T>>): Record<string, T> =>
    !retainedGraphPaths || Object.keys(values).every((path) => retainedGraphPaths.has(path))
      ? values
      : Object.fromEntries(Object.entries(values).filter(([path]) => retainedGraphPaths.has(path)));
  let state: GraphProjectionData = {
    graphEntities: retain(base.graphEntities),
    sessions: retain(base.sessions),
    resultStates: retain(base.resultStates),
  };
  const paths = new Set<string>();
  for (const { graphPath, session } of replacements) {
    if (paths.has(graphPath)) throw new Error(`Duplicate graph session '${graphPath}'`);
    paths.add(graphPath);
    state = prepareSession(state, graphPath, session);
  }
  return { graphPaths: [...paths], state };
}

interface GraphProjectionStore extends GraphProjectionData {
  getGraphNode(graphPath: GraphPath, nodeId: NodeId): NodeData | undefined;
  getGraphPin(graphPath: GraphPath, pinId: PinId): PinData | undefined;
  getGraphNodeIds(graphPath: GraphPath): NodeId[];
  getGraphNodePins(graphPath: GraphPath, nodeId: NodeId): PinId[];
  getGraphPinConnections(graphPath: GraphPath, pinId: PinId): ConnectionId[];
  getGraphConnection(graphPath: GraphPath, connectionId: ConnectionId): ConnectionData | undefined;
  hasGraph(graphPath: GraphPath): boolean;
  install(graphPath: string, session: GraphEditorSessionDto): void;
  hydrate(graphPath: string, session: GraphEditorSessionDto, saving?: boolean): void;
  setResultState(graphPath: string, result: GraphResultState): void;
  beginSave(graphPath: string): boolean;
  failSave(graphPath: string): void;
  clearGraph(graphPath: GraphPath): void;
  clear(): void;
}

export const useGraphProjectionStore = create<GraphProjectionStore>((set, get) => ({
  graphEntities: {},
  sessions: {},
  resultStates: {},
  getGraphNode: (graphPath, nodeId) => getGraphNode(get(), graphPath, nodeId),
  getGraphPin: (graphPath, pinId) => getGraphPin(get(), graphPath, pinId),
  getGraphNodeIds: (graphPath) => getGraphNodeIds(get(), graphPath),
  getGraphNodePins: (graphPath, nodeId) => getGraphNodePins(get(), graphPath, nodeId),
  getGraphPinConnections: (graphPath, pinId) => getGraphPinConnections(get(), graphPath, pinId),
  getGraphConnection: (graphPath, connectionId) =>
    getGraphConnection(get(), graphPath, connectionId),
  hasGraph: (graphPath) => hasGraphData(get(), graphPath),
  install: (path, input) => set((state) => prepareSession(state, path, input, false, true)),
  hydrate: (path, input, saving) => set((state) => prepareSession(state, path, input, saving)),
  setResultState: (path, result) =>
    set((state) => {
      if (
        !state.sessions[path] ||
        result.semanticInputHash !== state.sessions[path].semanticInputHash
      )
        return state;
      const current = state.resultStates[path];
      if (
        current &&
        current.executionSessionId === result.executionSessionId &&
        BigInt(current.revision) > BigInt(result.revision)
      )
        return state;
      const next = shareProjection(current, result);
      if (next === current) return state;
      const resultStates = { ...state.resultStates, [path]: next };
      freezePublishedValue(resultStates);
      return { resultStates };
    }),
  beginSave: (path) => {
    const current = get().sessions[path];
    if (!current || current.saving) return false;
    set((state) => ({ sessions: { ...state.sessions, [path]: { ...current, saving: true } } }));
    return true;
  },
  failSave: (path) =>
    set((state) => {
      const current = state.sessions[path];
      return current?.saving
        ? { sessions: { ...state.sessions, [path]: { ...current, saving: false } } }
        : state;
    }),
  clearGraph: (path) =>
    set((state) => {
      if (!state.sessions[path] && !state.graphEntities[path] && !state.resultStates[path])
        return state;
      const sessions = { ...state.sessions },
        graphEntities = { ...state.graphEntities },
        resultStates = { ...state.resultStates };
      delete sessions[path];
      delete graphEntities[path];
      delete resultStates[path];
      return { sessions, graphEntities, resultStates };
    }),
  clear: () => set({ sessions: {}, graphEntities: {}, resultStates: {} }),
}));

export function getGraphDocumentProjection(graphPath: string): GraphDocumentDto | null {
  return useGraphProjectionStore.getState().sessions[graphPath]?.document ?? null;
}
export function isGraphSaving(graphPath: string): boolean {
  return useGraphProjectionStore.getState().sessions[graphPath]?.saving === true;
}
export function isGraphModified(graphPath: string): boolean {
  return useGraphProjectionStore.getState().sessions[graphPath]?.saveDirty === true;
}
