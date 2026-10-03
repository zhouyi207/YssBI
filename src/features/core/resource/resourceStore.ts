import { freezePublishedValue } from "@/shared/types/deepReadonly";
import { produce, type Draft } from "immer";
import { create } from "zustand";
import { shallow } from "zustand/shallow";
import type { ProjectResourceMeta, ResourceKey, ResourceRef } from "./resourceTypes";
import { resourceKey } from "./resourceTypes";
import type { ChartDocument } from "@/shared/types/domain/chart";
import type { MindSnapshot } from "@/shared/types/domain/mind";
import type { DocSnapshot } from "@/shared/types/domain/doc";
import { shareGraphResultState } from "../dataStore/graphResultProjection";
import {
  sameChartDocument,
  shareChartDocument,
  shareDocSnapshot,
  shareMindSnapshot,
} from "./resourceDocumentProjection";
import type { GraphMeta } from "../dataStore/graphMeta";
import {
  canAcceptGraphSession,
  prepareGraphSessions,
  type GraphProjectionData,
} from "../dataStore/graphProjection";
import { lookupNodeFileResource } from "@/features/domain/resource/resourceQueries";
import type { DatabaseRecord, LoadDatabaseResult } from "@/shared/types/domain/database";
import { updateDatabaseColumns } from "@/features/core/database/databaseProjection";
import { logger } from "@/utils/frontendLogger";
import type { GraphEditorSessionDto } from "@/shared/types/domain/editorMutation";
import type { NodeFileKind } from "@/shared/types/domain/resource";
import type { GraphResultState } from "@/shared/types/domain/result";
import {
  getGraphConnection,
  getGraphNode,
  getGraphNodeIds,
  getGraphNodePins,
  getGraphPin,
  getGraphPinConnections,
  hasGraphData,
} from "../dataStore/graphEntityAccess";
import type {
  ConnectionData,
  ConnectionId,
  GraphPath,
  NodeData,
  NodeId,
  PinData,
  PinId,
} from "@/features/domain/editorProjection/graphRuntimeTypes";

export type GraphSessionPublication =
  | { mode: "load" | "update" }
  | { mode: "save"; resource: { kind: NodeFileKind; revision: number } };

export interface FileSnapshotByKind {
  mind: MindSnapshot;
  doc: DocSnapshot;
}
export type FileSnapshotKind = keyof FileSnapshotByKind;
export type FileSnapshots = { [K in FileSnapshotKind]: Record<string, FileSnapshotByKind[K]> };

export interface DocumentState {
  resourceKey: ResourceKey;
  loaded: boolean;
  dirty: boolean;
  stale: boolean;
  missing: boolean;
  conflict: boolean;
}

interface ResourceStore extends GraphProjectionData {
  databases: Record<string, DatabaseRecord>;
  graphMeta: Record<GraphPath, GraphMeta>;
  resources: Record<ResourceKey, ProjectResourceMeta>;
  documents: Record<ResourceKey, DocumentState>;
  chartDocuments: Record<string, ChartDocument>;
  fileSnapshots: FileSnapshots;
  graphOrder: string[];
  indexRevision: number;
  advancePublicationRevision(revision: number): void;
  setSnapshot(snapshot: {
    resources: ProjectResourceMeta[];
    graphOrder?: string[];
    publicationRevision?: number;
    documents?: Readonly<Record<ResourceKey, DocumentState>>;
    chartDocuments?: Readonly<Record<string, ChartDocument>>;
    fileSnapshots?: FileSnapshots;
    graphProjection?: GraphProjectionData;
    graphMeta?: Readonly<Record<GraphPath, GraphMeta>>;
    databases?: Readonly<Record<string, DatabaseRecord>>;
  }): void;
  updateDatabaseMetadata(
    id: string,
    expectedRevision: number,
    metadata: Omit<LoadDatabaseResult, "id" | "name">,
  ): void;
  patchResource(
    ref: ResourceRef,
    patch: Partial<Pick<ProjectResourceMeta, "name" | "revision" | "resourcePath" | "exists">>,
  ): void;
  upsertDocument(document: DocumentState): void;
  removeDocument(key: ResourceKey): void;
  markAllStale(): void;
  getGraphNode(graphPath: GraphPath, nodeId: NodeId): NodeData | undefined;
  getGraphPin(graphPath: GraphPath, pinId: PinId): PinData | undefined;
  getGraphNodeIds(graphPath: GraphPath): NodeId[];
  getGraphNodePins(graphPath: GraphPath, nodeId: NodeId): PinId[];
  getGraphPinConnections(graphPath: GraphPath, pinId: PinId): ConnectionId[];
  getGraphConnection(graphPath: GraphPath, connectionId: ConnectionId): ConnectionData | undefined;
  hasGraph(graphPath: GraphPath): boolean;
  installGraphSession(
    graphPath: string,
    session: GraphEditorSessionDto,
    options?: GraphSessionPublication,
  ): boolean;
  setGraphResultState(graphPath: string, result: GraphResultState): void;
  beginGraphSave(graphPath: string): boolean;
  failGraphSave(graphPath: string): void;
  removeGraphSession(graphPath: GraphPath): void;
  beginFileRead(kind: FileSnapshotKind, path: string): () => boolean;
  installFileSnapshot(
    snapshot: MindSnapshot | DocSnapshot,
    options?: { pendingInput?: boolean; renamedFrom?: string },
  ): boolean;
  removeFileSnapshot(kind: FileSnapshotKind, path: string): void;
  beginChartRead(path: string): () => boolean;
  upsertChartDocument(path: string, document: ChartDocument): void;
  updateChartDocument(path: string, patch: Partial<ChartDocument>): ChartDocument | null;
  removeChartDocument(path: string): void;
  settleChartSave(
    path: string,
    submitted: ChartDocument,
    expected: ChartDocument,
    revision: number,
  ): boolean;
  clear(): void;
}

export function createDocumentState(key: ResourceKey): DocumentState {
  return {
    resourceKey: key,
    loaded: false,
    dirty: false,
    stale: false,
    missing: false,
    conflict: false,
  };
}

export function withDirtyState(previous: DocumentState, dirty: boolean): DocumentState {
  return {
    ...previous,
    loaded: true,
    dirty,
    stale: dirty ? previous.stale : false,
    conflict: dirty ? previous.conflict : false,
  };
}

function publishDocumentState(
  state: ResourceStore,
  document: DocumentState,
  revision?: number,
): void {
  const key = document.resourceKey;
  // Entries installed during this transaction may be frozen snapshot inputs, not drafts.
  if (!shallow(state.documents[key], document)) state.documents[key] = document;
  const resource = state.resources[key];
  if (resource) {
    const next = {
      ...resource,
      ...documentSummary(document),
      ...(revision === undefined ? {} : { revision }),
    };
    if (!shallow(resource, next)) state.resources[key] = next;
  }
}

function removeDocumentState(state: ResourceStore, key: ResourceKey): void {
  delete state.documents[key];
  const resource = state.resources[key];
  if (resource) {
    const next = { ...resource, ...documentSummary(undefined) };
    if (!shallow(resource, next)) state.resources[key] = next;
  }
}

function publishChartState(
  state: ResourceStore,
  path: string,
  patch: Partial<Pick<DocumentState, "dirty" | "stale" | "conflict">> = {},
): void {
  const key = resourceKey({ kind: "chart", id: path });
  const previous = state.documents[key];
  const document = {
    ...(previous ?? createDocumentState(key)),
    loaded: true,
    missing: state.resources[key]?.exists === false,
    ...patch,
  };
  publishDocumentState(state, document);
}

const chartReads = new Map<string, object>();
const fileReads = new Map<ResourceKey, object>();

function documentSummary(document: DocumentState | undefined) {
  return {
    loaded: document?.loaded ?? false,
    hasDirtyDocument: document?.dirty ?? false,
    hasStaleDocument: document?.stale ?? false,
    hasConflictDocument: document?.conflict ?? false,
  };
}

export const useResourceStore = create<ResourceStore>((set, get) => ({
  databases: {},
  graphMeta: {},
  graphEntities: {},
  sessions: {},
  resultStates: {},
  resources: {},
  documents: {},
  chartDocuments: {},
  fileSnapshots: { mind: {}, doc: {} },
  graphOrder: [],
  indexRevision: 0,

  advancePublicationRevision: (revision) =>
    set((state) => (revision > state.indexRevision ? { indexRevision: revision } : state)),

  setSnapshot: ({
    resources,
    graphOrder,
    publicationRevision,
    documents,
    chartDocuments,
    fileSnapshots,
    graphProjection,
    graphMeta,
    databases,
  }) =>
    set(
      produce((state: ResourceStore) => {
        if (databases) state.databases = databases;
        if (graphMeta) state.graphMeta = graphMeta;
        const previousGraphPaths = graphProjection ? Object.keys(state.sessions) : [];
        if (graphProjection) {
          state.sessions = graphProjection.sessions;
          state.graphEntities = graphProjection.graphEntities;
          state.resultStates = graphProjection.resultStates;
        }
        if (fileSnapshots) {
          for (const kind of ["mind", "doc"] as const)
            for (const path of Object.keys(state.fileSnapshots[kind]))
              if (!Object.prototype.hasOwnProperty.call(fileSnapshots[kind], path)) {
                delete state.fileSnapshots[kind][path];
                fileReads.delete(resourceKey({ kind, id: path }));
              }
          for (const [path, snapshot] of Object.entries(fileSnapshots.mind)) {
            const previous = state.fileSnapshots.mind[path];
            const next = shareMindSnapshot(previous, snapshot);
            if (next === previous) continue;
            state.fileSnapshots.mind[path] = next;
            if (previous) fileReads.delete(resourceKey({ kind: "mind", id: path }));
          }
          for (const [path, snapshot] of Object.entries(fileSnapshots.doc)) {
            const previous = state.fileSnapshots.doc[path];
            const next = shareDocSnapshot(previous, snapshot);
            if (next === previous) continue;
            state.fileSnapshots.doc[path] = next;
            if (previous) fileReads.delete(resourceKey({ kind: "doc", id: path }));
          }
        }
        if (chartDocuments) {
          for (const path of Object.keys(state.chartDocuments)) {
            if (Object.prototype.hasOwnProperty.call(chartDocuments, path)) continue;
            delete state.chartDocuments[path];
            chartReads.delete(path);
          }
          for (const [path, document] of Object.entries(chartDocuments)) {
            const previous = state.chartDocuments[path];
            const next = shareChartDocument(previous, document);
            if (next === previous) continue;
            state.chartDocuments[path] = next;
            chartReads.delete(path);
          }
        }
        if (documents) {
          for (const key of Object.keys(state.documents)) {
            if (!Object.prototype.hasOwnProperty.call(documents, key)) delete state.documents[key];
          }
          for (const [key, document] of Object.entries(documents)) {
            if (!shallow(state.documents[key], document)) state.documents[key] = document;
          }
        }
        const incomingKeys = new Set<ResourceKey>();
        for (const resource of resources) {
          const key = resourceKey(resource);
          incomingKeys.add(key);
          const document = state.documents[key];
          const next = document ? { ...resource, ...documentSummary(document) } : resource;
          if (!shallow(state.resources[key], next)) state.resources[key] = next;
        }
        for (const key of Object.keys(state.resources)) {
          if (!incomingKeys.has(key)) delete state.resources[key];
        }
        if (chartDocuments)
          for (const path of Object.keys(state.chartDocuments)) publishChartState(state, path);
        if (graphProjection) {
          for (const path of previousGraphPaths) {
            if (graphProjection.sessions[path]) continue;
            for (const kind of ["event_graph", "function_graph"] as const)
              removeDocumentState(state, resourceKey({ kind, id: path }));
          }
          for (const resource of Object.values(state.resources)) {
            if (resource.kind !== "event_graph" && resource.kind !== "function_graph") continue;
            const key = resourceKey(resource);
            const session = state.sessions[resource.id];
            const previous = state.documents[key];
            if (!session && !previous) {
              removeDocumentState(state, key);
              continue;
            }
            publishDocumentState(state, {
              ...(previous ?? createDocumentState(key)),
              loaded: !!session,
              missing: resource.exists === false,
              dirty: session?.saveDirty ?? previous?.dirty ?? false,
            });
          }
        }
        if (fileSnapshots) {
          for (const resource of Object.values(state.resources)) {
            if (resource.kind !== "mind" && resource.kind !== "doc") continue;
            const key = resourceKey(resource);
            const snapshot = state.fileSnapshots[resource.kind][resource.id];
            const previous = state.documents[key];
            if (!snapshot && !previous) {
              removeDocumentState(state, key);
              continue;
            }
            publishDocumentState(state, {
              ...(previous ?? createDocumentState(key)),
              loaded: !!snapshot,
              missing: resource.exists === false,
              dirty: snapshot?.dirty || previous?.dirty || false,
            });
          }
        }
        const order =
          graphOrder ??
          resources
            .filter(
              (resource) => resource.kind === "event_graph" || resource.kind === "function_graph",
            )
            .map((resource) => resource.id);
        if (!shallow(state.graphOrder, order)) state.graphOrder = order;
        state.indexRevision = publicationRevision ?? state.indexRevision;
      }),
    ),

  patchResource: (ref, patch) =>
    set(
      produce((state: ResourceStore) => {
        const resource = state.resources[resourceKey(ref)];
        if (resource) Object.assign(resource, patch);
      }),
    ),

  upsertDocument: (document) =>
    set(
      produce((state: ResourceStore) => {
        publishDocumentState(state, document);
      }),
    ),

  removeDocument: (key) =>
    set(
      produce((state: ResourceStore) => {
        removeDocumentState(state, key);
      }),
    ),

  markAllStale: () =>
    set(
      produce((state: ResourceStore) => {
        for (const document of Object.values(state.documents)) document.stale = true;
        for (const resource of Object.values(state.resources)) resource.hasStaleDocument = true;
      }),
    ),

  getGraphNode: (graphPath, nodeId) => getGraphNode(get(), graphPath, nodeId),
  getGraphPin: (graphPath, pinId) => getGraphPin(get(), graphPath, pinId),
  getGraphNodeIds: (graphPath) => getGraphNodeIds(get(), graphPath),
  getGraphNodePins: (graphPath, nodeId) => getGraphNodePins(get(), graphPath, nodeId),
  getGraphPinConnections: (graphPath, pinId) => getGraphPinConnections(get(), graphPath, pinId),
  getGraphConnection: (graphPath, connectionId) =>
    getGraphConnection(get(), graphPath, connectionId),
  hasGraph: (graphPath) => hasGraphData(get(), graphPath),

  installGraphSession: (graphPath, session, options = { mode: "update" }) => {
    const current = get();
    if (!canAcceptGraphSession(current.sessions[graphPath], session)) return false;
    const prepared = prepareGraphSessions(
      [
        {
          graphPath,
          session,
          saving: options.mode === "update" ? undefined : false,
          renew: options.mode === "load",
        },
      ],
      undefined,
      current,
    ).state;
    const resource =
      options.mode === "save"
        ? options.resource
        : lookupNodeFileResource(current.resources, graphPath);
    const revision =
      options.mode === "save"
        ? options.resource.revision
        : Number(session.editing.version.revision);
    set(
      produce((state: ResourceStore) => {
        state.sessions = prepared.sessions;
        state.graphEntities = prepared.graphEntities;
        state.resultStates = prepared.resultStates;
        if (resource) {
          const key = resourceKey({ kind: resource.kind, id: graphPath });
          publishDocumentState(
            state,
            {
              ...withDirtyState(
                state.documents[key] ?? createDocumentState(key),
                session.editing.dirty,
              ),
              missing: state.resources[key]?.exists === false,
              stale: false,
              conflict: false,
            },
            Number.isSafeInteger(revision) ? revision : undefined,
          );
        }
      }),
    );
    // Starting the next save changes only its lock; reopening or publishing another
    // frame changes the existing session/generation identity.
    const installed = get().sessions[graphPath];
    const accepted = prepared.sessions[graphPath];
    return (
      installed?.sessionId === accepted.sessionId &&
      installed.projectionGeneration === accepted.projectionGeneration
    );
  },

  setGraphResultState: (path, result) => {
    const current = get();
    if (
      !current.sessions[path] ||
      result.semanticInputHash !== current.sessions[path].semanticInputHash
    )
      return;
    const previous = current.resultStates[path];
    if (
      previous &&
      previous.executionSessionId === result.executionSessionId &&
      BigInt(previous.revision) > BigInt(result.revision)
    )
      return;
    const next = shareGraphResultState(previous, result);
    if (next === previous) return;
    set({ resultStates: freezePublishedValue({ ...current.resultStates, [path]: next }) });
  },

  beginGraphSave: (path) => {
    const current = get().sessions[path];
    if (!current || current.saving) return false;
    set(
      produce((state: Draft<ResourceStore>) => {
        state.sessions[path].saving = true;
      }),
    );
    return true;
  },

  failGraphSave: (path) =>
    set(
      produce((state: Draft<ResourceStore>) => {
        if (state.sessions[path]?.saving) state.sessions[path].saving = false;
      }),
    ),

  removeGraphSession: (path) =>
    set(
      produce((state: Draft<ResourceStore>) => {
        delete state.sessions[path];
        delete state.graphEntities[path];
        delete state.resultStates[path];
        for (const kind of ["event_graph", "function_graph"] as const)
          removeDocumentState(state, resourceKey({ kind, id: path }));
      }),
    ),

  beginFileRead: (kind, path) => {
    const key = resourceKey({ kind, id: path });
    const token = {};
    const resource = get().resources[key];
    fileReads.set(key, token);
    return () => {
      const current = get().resources[key];
      return (
        fileReads.get(key) === token &&
        current?.revision === resource?.revision &&
        current?.exists === resource?.exists
      );
    };
  },

  installFileSnapshot: (snapshot, { pendingInput = false, renamedFrom } = {}) => {
    const previous = get().fileSnapshots[snapshot.kind][snapshot.path];
    const outdated =
      previous?.projectInstanceId === snapshot.projectInstanceId &&
      previous.version.sessionId === snapshot.version.sessionId &&
      previous.version.revision > snapshot.version.revision;
    if (outdated && !renamedFrom) return false;
    const key = resourceKey({ kind: snapshot.kind, id: snapshot.path });
    if (!outdated) fileReads.delete(key);
    set(
      produce((state: ResourceStore) => {
        if (renamedFrom && renamedFrom !== snapshot.path) {
          const previousKey = resourceKey({ kind: snapshot.kind, id: renamedFrom });
          fileReads.delete(previousKey);
          delete state.fileSnapshots[snapshot.kind][renamedFrom];
          removeDocumentState(state, previousKey);
        }
        if (outdated) return;
        if (snapshot.kind === "mind")
          state.fileSnapshots.mind[snapshot.path] = shareMindSnapshot(
            state.fileSnapshots.mind[snapshot.path],
            snapshot,
          );
        else
          state.fileSnapshots.doc[snapshot.path] = shareDocSnapshot(
            state.fileSnapshots.doc[snapshot.path],
            snapshot,
          );
        publishDocumentState(
          state,
          {
            ...withDirtyState(
              state.documents[key] ?? createDocumentState(key),
              snapshot.dirty || pendingInput,
            ),
            missing: state.resources[key]?.exists === false,
          },
          snapshot.version.revision,
        );
      }),
    );
    return !outdated;
  },

  removeFileSnapshot: (kind, path) => {
    const key = resourceKey({ kind, id: path });
    fileReads.delete(key);
    set(
      produce((state: ResourceStore) => {
        delete state.fileSnapshots[kind][path];
        removeDocumentState(state, key);
      }),
    );
  },

  beginChartRead: (path) => {
    const token = {};
    chartReads.set(path, token);
    return () => chartReads.get(path) === token;
  },

  upsertChartDocument: (path, document) => {
    chartReads.delete(path);
    set(
      produce((state: ResourceStore) => {
        state.chartDocuments[path] = shareChartDocument(state.chartDocuments[path], document);
        publishChartState(state, path);
      }),
    );
  },

  updateChartDocument: (path, patch) => {
    const current = get().chartDocuments[path];
    if (!current) return null;
    const next = shareChartDocument(current, {
      ...current,
      ...patch,
      encodings: { ...current.encodings, ...patch.encodings },
    });
    if (next === current) return current;
    chartReads.delete(path);
    set(
      produce((state: ResourceStore) => {
        state.chartDocuments[path] = next;
        publishChartState(state, path, { dirty: true });
      }),
    );
    return next;
  },

  removeChartDocument: (path) => {
    chartReads.delete(path);
    set(
      produce((state: ResourceStore) => {
        delete state.chartDocuments[path];
        const key = resourceKey({ kind: "chart", id: path });
        removeDocumentState(state, key);
      }),
    );
  },

  settleChartSave: (path, submitted, expected, revision) => {
    let saved = false;
    set(
      produce((state: ResourceStore) => {
        const current = state.chartDocuments[path];
        const resource = state.resources[resourceKey({ kind: "chart", id: path })];
        if (!current || !resource?.exists || resource.revision !== revision) return;
        if (sameChartDocument(current, submitted))
          state.chartDocuments[path] = shareChartDocument(current, expected);
        saved = sameChartDocument(state.chartDocuments[path], expected);
        publishChartState(state, path, { dirty: !saved, stale: false, conflict: false });
        chartReads.delete(path);
      }),
    );
    return saved;
  },

  updateDatabaseMetadata: (id, expectedRevision, metadata) => {
    const snapshot = get();
    const previous = snapshot.databases[id];
    if (!previous) {
      logger.data.warn(`updateDatabaseMetadata: id "${id}" not found`, "ResourceStore");
      return;
    }
    const resource = snapshot.resources[resourceKey({ kind: "database", id })];
    if (!resource?.exists || resource.revision !== expectedRevision) return;
    set(
      produce((state: Draft<ResourceStore>) => {
        const database = state.databases[id];
        updateDatabaseColumns(database, metadata.columns);
        database.rowCount = metadata.rowCount;
        database.columnCount = metadata.columnCount;
      }),
    );
  },

  clear: () => {
    chartReads.clear();
    fileReads.clear();
    set(
      produce((state: ResourceStore) => {
        if (Object.keys(state.databases).length) state.databases = {};
        if (Object.keys(state.graphMeta).length) state.graphMeta = {};
        if (Object.keys(state.sessions).length) state.sessions = {};
        if (Object.keys(state.graphEntities).length) state.graphEntities = {};
        if (Object.keys(state.resultStates).length) state.resultStates = {};
        if (Object.keys(state.resources).length) state.resources = {};
        if (Object.keys(state.documents).length) state.documents = {};
        if (Object.keys(state.chartDocuments).length) state.chartDocuments = {};
        if (Object.keys(state.fileSnapshots.mind).length) state.fileSnapshots.mind = {};
        if (Object.keys(state.fileSnapshots.doc).length) state.fileSnapshots.doc = {};
        if (state.graphOrder.length) state.graphOrder = [];
        state.indexRevision = 0;
      }),
    );
  },
}));
