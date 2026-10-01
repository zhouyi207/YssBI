import { freezePublishedValue } from "@/shared/types/deepReadonly";
import { produce } from "immer";
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
import type {
  EditorNodeProjectionDto,
  ParameterGroupDto,
} from "@/shared/types/domain/editorProjection";
import type { GraphResultState } from "@/shared/types/domain/result";
import { portAddressKey } from "@/features/domain/editorProjection";
import { validateEditorGraphProjection } from "@/shared/types/domain/editorProjectionParser";
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

function shareItems<T>(previous: T[] | undefined, next: T[]): T[] {
  return previous?.length === next.length && next.every((id, i) => id === previous[i])
    ? previous
    : next;
}

function shareParameterGroups(
  previous: ParameterGroupDto[] | undefined,
  next: ParameterGroupDto[],
): ParameterGroupDto[] {
  if (!previous || previous === next) return next;
  const groups = new Map(previous.map((group) => [group.key, group]));
  return shareItems(
    previous,
    next.map((group) => {
      const before = groups.get(group.key);
      if (!before || before === group) return group;
      const parameters = new Map(before.parameters.map((parameter) => [parameter.key, parameter]));
      const shared = {
        ...group,
        display: shareProjection(before.display, group.display),
        parameters: shareItems(
          before.parameters,
          group.parameters.map((parameter) =>
            shareProjection(parameters.get(parameter.key), parameter),
          ),
        ),
      };
      return sameFields(before, shared) ? before : shared;
    }),
  );
}

function buildProjectionBucket(
  graphPath: string,
  projection: EditorGraphProjectionDto,
  previous?: GraphEntityBucket,
  previousProjection?: EditorGraphProjectionDto,
): GraphEntityBucket {
  // Keep the whole-graph identity/endpoint checks before publishing any derived indexes.
  validateEditorGraphProjection(projection);
  if (projection.graphPath !== graphPath)
    throw new Error(`Projection '${projection.graphPath}' does not match '${graphPath}'`);
  const base: GraphEntityBucket = previous ?? {
    basis: projection.basis,
    diagnostics: projection.diagnostics,
    outcome: projection.outcome,
    hasBlockingDiagnostics: projection.hasBlockingDiagnostics,
    nodes: Object.create(null),
    pins: Object.create(null),
    connections: Object.create(null),
    graphNodes: [],
    pinConnections: Object.create(null),
    blockedConnectionIds: Object.create(null),
    primaryPortDiagnostics: Object.create(null),
  };
  const update = (bucket: GraphEntityBucket) => {
    const clearPortDiagnostics = (node: NodeData) => {
      for (const diagnostic of node.diagnostics) {
        const location = diagnostic.location;
        if (location.kind === "port" && location.address.nodeId === node.id)
          delete bucket.primaryPortDiagnostics[portAddressKey(location.address)];
      }
    };
    bucket.basis = projection.basis;
    bucket.diagnostics = projection.diagnostics;
    bucket.outcome = projection.outcome;
    bucket.hasBlockingDiagnostics = projection.hasBlockingDiagnostics;
    if (!previous || projection.diagnostics !== previous.diagnostics) {
      const blocked: Record<ConnectionId, true> = Object.create(null);
      for (const diagnostic of projection.diagnostics) {
        if (diagnostic.blocking && diagnostic.location.kind === "connection")
          blocked[diagnostic.location.connectionId] = true;
      }
      bucket.blockedConnectionIds = shareEntity(previous?.blockedConnectionIds, blocked);
    }
    // Read the immutable source, not draft dictionaries: untouched entries need no proxies.
    if (!previous || projection.nodes !== previousProjection?.nodes) {
      const ids = shareItems(
        base.graphNodes,
        projection.nodes.map((node) => node.nodeId),
      );
      bucket.graphNodes = ids;
      let previousNodesById: Map<NodeId, EditorNodeProjectionDto> | undefined;
      for (const [index, node] of projection.nodes.entries()) {
        const before = base.nodes[node.nodeId];
        const atIndex = previousProjection?.nodes[index];
        const source =
          atIndex?.nodeId === node.nodeId
            ? atIndex
            : previousProjection &&
              (previousNodesById ??= new Map(
                previousProjection.nodes.map((entry) => [entry.nodeId, entry]),
              )).get(node.nodeId);
        if (before && node === source) continue;
        let pinIds = before?.pinIds ?? [];
        if (!before || node.ports !== source?.ports) {
          pinIds = shareItems(
            before?.pinIds,
            node.ports.map((port) => portAddressKey(port.address)),
          );
          for (const [portIndex, port] of node.ports.entries()) {
            const id = pinIds[portIndex];
            bucket.pins[id] = shareEntity(base.pins[id], {
              id,
              nodeId: port.address.nodeId,
              name: port.display.instanceLabel ?? port.display.label,
              direction: port.direction,
              address: port.address,
              display: port.display,
              orphan: port.orphan,
              canRemove: port.canRemove,
              connections: port.connections,
              input: port.input,
              typeState: port.typeState,
              resolvedSchema: port.resolvedSchema,
              status: port.status,
            });
            if (!base.pinConnections[id]) bucket.pinConnections[id] = [];
          }
          if (before && pinIds !== before.pinIds) {
            const retained = new Set(pinIds);
            for (const id of before.pinIds) {
              if (retained.has(id)) continue;
              delete bucket.pins[id];
              delete bucket.pinConnections[id];
            }
          }
        }
        const shared: NodeData = {
          id: node.nodeId,
          graphPath: node.graphPath,
          nodeType: node.nodeTypeId,
          position: shareProjection(before?.position, node.position),
          pinIds,
          display: shareProjection(before?.display, node.display),
          parameterGroups: shareParameterGroups(before?.parameterGroups, node.parameterGroups),
          portInstanceAdditions: shareProjection(
            before?.portInstanceAdditions,
            node.portInstanceAdditions,
          ),
          capabilities: shareProjection(before?.capabilities, node.capabilities),
          diagnostics: shareProjection(before?.diagnostics, node.diagnostics),
        };
        bucket.nodes[node.nodeId] = sameFields(before, shared) ? before : shared;
        if (!before || shared.diagnostics !== before.diagnostics) {
          // Fresh nodes populate the graph index directly; changed nodes stage only their entries.
          const diagnostics: GraphEntityBucket["primaryPortDiagnostics"] = before
            ? Object.create(null)
            : bucket.primaryPortDiagnostics;
          for (const diagnostic of shared.diagnostics) {
            const location = diagnostic.location;
            if (location.kind !== "port" || location.address.nodeId !== shared.id) continue;
            const id = portAddressKey(location.address);
            const selected = diagnostics[id];
            // Preserve the old per-node lookup: first blocking, otherwise first matching.
            if (!selected || (!selected.blocking && diagnostic.blocking))
              diagnostics[id] = diagnostic;
          }
          if (before) {
            for (const diagnostic of before.diagnostics) {
              const location = diagnostic.location;
              if (location.kind !== "port" || location.address.nodeId !== before.id) continue;
              const id = portAddressKey(location.address);
              if (!diagnostics[id]) delete bucket.primaryPortDiagnostics[id];
            }
            for (const id in diagnostics) {
              if (base.primaryPortDiagnostics[id] !== diagnostics[id])
                bucket.primaryPortDiagnostics[id] = diagnostics[id];
            }
          }
        }
      }
      if (ids !== base.graphNodes) {
        const retained = new Set(ids);
        for (const id of base.graphNodes) {
          if (retained.has(id)) continue;
          for (const pinId of base.nodes[id].pinIds) {
            delete bucket.pins[pinId];
            delete bucket.pinConnections[pinId];
          }
          clearPortDiagnostics(base.nodes[id]);
          delete bucket.nodes[id];
        }
      }
    }
    if (!previous || projection.connections !== previousProjection?.connections) {
      const retained = new Set<ConnectionId>();
      const affectedPorts = new Set<PinId>();
      const connections: ConnectionData[] = [];
      for (const [index, connection] of projection.connections.entries()) {
        const id = connection.connectionId;
        retained.add(id);
        const existing = base.connections[id];
        const atIndex = previousProjection?.connections[index];
        const from =
          existing && connection === atIndex ? existing.from : portAddressKey(connection.output);
        const to =
          existing && connection === atIndex ? existing.to : portAddressKey(connection.input);
        const shared = shareEntity(existing, {
          id,
          from,
          to,
          output: existing?.from === from ? existing.output : connection.output,
          input: existing?.to === to ? existing.input : connection.input,
          order: connection.order,
        });
        bucket.connections[id] = shared;
        connections.push(shared);
        if (atIndex?.connectionId !== id || existing?.from !== from || existing?.to !== to) {
          affectedPorts.add(from).add(to);
          if (existing) affectedPorts.add(existing.from).add(existing.to);
        }
      }
      for (const connection of previousProjection?.connections ?? []) {
        if (retained.has(connection.connectionId)) continue;
        const existing = base.connections[connection.connectionId];
        affectedPorts.add(existing.from).add(existing.to);
        delete bucket.connections[connection.connectionId];
      }
      // Only affected adjacency lists are rebuilt. Scan source order to preserve reordering,
      // including duplicate endpoints on a diagnostic-blocked self connection.
      const adjacency = new Map([...affectedPorts].map((id) => [id, [] as ConnectionId[]]));
      for (const connection of connections) {
        adjacency.get(connection.from)?.push(connection.id);
        adjacency.get(connection.to)?.push(connection.id);
      }
      for (const [id, connectionIds] of adjacency) {
        if (bucket.pins[id])
          bucket.pinConnections[id] = shareItems(base.pinConnections[id], connectionIds);
      }
    }
  };
  if (previous) return produce(base, update);
  // A fresh table has no readers yet. Populate it before the existing publication freeze,
  // avoiding both per-entry proxies and a second deep freeze during initial load.
  update(base);
  return base;
}

let nextViewSessionId = 0;

interface PreparedGraphSession {
  session: GraphEditorState;
  bucket: GraphEntityBucket;
  resultState: GraphResultState;
}

function prepareSession(
  state: GraphProjectionData,
  graphPath: string,
  input: GraphEditorSessionDto,
  saving?: boolean,
  renew = false,
): PreparedGraphSession | undefined {
  const previous = state.sessions[graphPath];
  if (!canAcceptGraphSession(previous, input)) return;
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
  if (sessionUnchanged && resultState === state.resultStates[graphPath]) return;
  const bucket =
    projection === previous?.projection && state.graphEntities[graphPath]
      ? state.graphEntities[graphPath]
      : buildProjectionBucket(
          graphPath,
          projection,
          state.graphEntities[graphPath],
          previous?.projection,
        );
  return {
    session: sessionUnchanged
      ? previous
      : {
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
    bucket,
    resultState,
  };
}

export interface PreparedGraphSessions {
  readonly graphPaths: readonly string[];
  readonly state: GraphProjectionData;
}

/** Prepare all accepted sessions before a single publication, including project snapshots. */
export function prepareGraphSessions(
  replacements: readonly {
    graphPath: string;
    session: GraphEditorSessionDto;
    saving?: boolean;
    renew?: boolean;
  }[],
  retainedGraphPaths?: ReadonlySet<string>,
  base: GraphProjectionData = useGraphProjectionStore.getState(),
): PreparedGraphSessions {
  const retain = <T>(values: Readonly<Record<string, T>>): Record<string, T> =>
    !retainedGraphPaths || Object.keys(values).every((path) => retainedGraphPaths.has(path))
      ? values
      : Object.fromEntries(Object.entries(values).filter(([path]) => retainedGraphPaths.has(path)));
  const retained: GraphProjectionData = {
    graphEntities: retain(base.graphEntities),
    sessions: retain(base.sessions),
    resultStates: retain(base.resultStates),
  };
  let sessions: Record<string, GraphEditorState> = retained.sessions;
  let graphEntities = retained.graphEntities;
  let resultStates: Record<string, GraphResultState> = retained.resultStates;
  const paths = new Set<string>();
  for (const { graphPath, session, saving, renew } of replacements) {
    if (paths.has(graphPath)) throw new Error(`Duplicate graph session '${graphPath}'`);
    paths.add(graphPath);
    const prepared = prepareSession(retained, graphPath, session, saving, renew);
    if (!prepared) continue;
    // Each project-level table is copied at most once. Tables already filtered by retain
    // are owned by this unpublished batch; entity updates still use their existing owner.
    if (prepared.session !== sessions[graphPath]) {
      if (sessions === base.sessions) sessions = { ...sessions };
      sessions[graphPath] = prepared.session;
    }
    if (prepared.bucket !== graphEntities[graphPath]) {
      if (graphEntities === base.graphEntities) graphEntities = { ...graphEntities };
      graphEntities[graphPath] = prepared.bucket;
    }
    if (prepared.resultState !== resultStates[graphPath]) {
      if (resultStates === base.resultStates) resultStates = { ...resultStates };
      resultStates[graphPath] = prepared.resultState;
    }
  }
  const state =
    sessions === base.sessions &&
    graphEntities === base.graphEntities &&
    resultStates === base.resultStates
      ? base
      : { sessions, graphEntities, resultStates };
  if (state !== base) freezePublishedValue(state);
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
  install: (graphPath, session) =>
    set(
      (state) =>
        prepareGraphSessions([{ graphPath, session, saving: false, renew: true }], undefined, state)
          .state,
    ),
  hydrate: (graphPath, session, saving) =>
    set((state) => prepareGraphSessions([{ graphPath, session, saving }], undefined, state).state),
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
