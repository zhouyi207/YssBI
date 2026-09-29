import { nodeFileEntries } from "@/shared/types/domain/project";
import {
  detailResource,
  detailResourceRef,
} from "@/features/core/editor/detail/editorDetailPolicy";
import {
  hasPendingDocumentInput,
  remapDocumentInputs,
} from "@/features/application/resource/documentInputs";
import { shouldRetainResourceEditor } from "@/features/core/resource";
import { useSidebarStore } from "@/features/core/sidebar/sidebarStore";
import type { ResourceMutationResultDto } from "@/shared/types/domain/editorMutation";
import type { ProjectDatabaseIndexRow, ProjectIndexRow } from "@/shared/types/domain/project";
import { parseProjectIndexRow } from "@/services/project/projectService";
import type { DatabaseRecord } from "@/shared/types/domain/database";
import type {
  PreparedProjectSnapshot,
  ProjectSnapshotPreparation,
} from "./projectPublicationCoordinator";
import { useDatabaseStore, useGraphMetaStore } from "@/features/core/dataStore";
import {
  prepareGraphSessions,
  useGraphProjectionStore,
  canAcceptGraphSession,
  isGraphModified,
  isGraphSaving,
} from "@/features/core/dataStore/graphProjectionStore";

import {
  assertCurrentProjectIdentity,
  isCurrentProjectIdentity,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { useChartDocumentStore } from "@/features/core/chart/chartDocumentStore";
import { useMindProjectionStore } from "@/features/core/resource/mindProjectionStore";
import { useDocProjectionStore } from "@/features/core/resource/docProjectionStore";
import {
  prepareResourceProjectionSnapshot,
  resourceKey,
  useDocumentStateStore,
  useResourceStore,
  type DocumentState,
  type ProjectResourceMeta,
  type ResourceKey,
} from "@/features/core/resource";
import { useGraphSessionStore } from "@/features/core/graphSession/graphSessionStore";
import { useEditorStore } from "@/features/core/editor/stores/useEditorStore";
import { useViewportStore } from "@/features/core/viewport";
import { parseViewportScopeKey, viewportScopeKey } from "@/features/core/viewport/viewportScope";
import {
  remapGraphNonViewportUiState,
  remapFileNonViewportUiState,
} from "@/features/application/editor/cascadeGraphPathReferences";
import { invalidateChartPreviewCacheForMove } from "@/services/chart/chartPreviewCache";
import { commitEditorLayoutPublication } from "./editorLayoutPublicationCommit";
import { buildProjectResourceState } from "@/features/application/project/authoritativeProjectLoadPlan";
import { reconcileGraphResultQueries } from "@/features/application/results/runtime";
import { releaseDocumentInputs } from "@/features/application/resource/documentInputs";

export function validateProjectSnapshotIndex(
  index: ProjectIndexRow,
  projectInstanceId: string,
): string | undefined {
  if (index.projectInstanceId !== projectInstanceId) return "snapshot project identity is stale";
  try {
    parseProjectIndexRow(index);
  } catch {
    return "project index contract is malformed";
  }
  return undefined;
}

function canReach(
  movesBySource: ReadonlyMap<string, ReadonlySet<string>>,
  from: string,
  destination: string,
): boolean {
  const pending = [from];
  const visited = new Set<string>();
  while (pending.length > 0) {
    const path = pending.pop() as string;
    if (path === destination) return true;
    if (visited.has(path)) continue;
    visited.add(path);
    pending.push(...(movesBySource.get(path) ?? []));
  }
  return false;
}

function authoritativeTerminals(
  authoritativeGraphPaths: ReadonlySet<string>,
  movesBySource: ReadonlyMap<string, ReadonlySet<string>>,
  source: string,
): ReadonlySet<string> {
  const terminals = new Set<string>();
  const pending = [...(movesBySource.get(source) ?? [])];
  const visited = new Set<string>([source]);
  while (pending.length > 0) {
    const path = pending.pop() as string;
    if (authoritativeGraphPaths.has(path)) {
      terminals.add(path);
      continue;
    }
    if (visited.has(path)) continue;
    visited.add(path);
    pending.push(...(movesBySource.get(path) ?? []));
  }
  return terminals;
}

function buildSnapshotPathRemaps(
  authoritativePaths: ReadonlySet<string>,
  queuedResults: readonly ResourceMutationResultDto[],
  accepts: (move: ResourceMutationResultDto["moves"][number]) => boolean,
): ReadonlyMap<string, string> {
  const movesBySource = new Map<string, Set<string>>();
  for (const result of queuedResults) {
    for (const move of result.moves) {
      if (!accepts(move)) continue;
      const destinations = movesBySource.get(move.from) ?? new Set<string>();
      destinations.add(move.to);
      movesBySource.set(move.from, destinations);
    }
  }

  const destinationOwners = new Map<string, string>();
  const pathRemaps = new Map<string, string>();
  for (const source of movesBySource.keys()) {
    if (authoritativePaths.has(source)) continue;
    const terminals = [...authoritativeTerminals(authoritativePaths, movesBySource, source)];
    if (terminals.length > 1) {
      throw new Error(`conflicting recovery move source '${source}'`);
    }
    const terminal = terminals[0];
    if (!terminal) continue;
    const destinationOwner = destinationOwners.get(terminal);
    if (
      destinationOwner &&
      !canReach(movesBySource, destinationOwner, source) &&
      !canReach(movesBySource, source, destinationOwner)
    ) {
      throw new Error(`conflicting recovery move destination '${terminal}'`);
    }
    pathRemaps.set(source, terminal);
    destinationOwners.set(terminal, destinationOwner ?? source);
  }
  return pathRemaps;
}

export function buildProjectSnapshotPathRemaps(
  authoritativeGraphPaths: ReadonlySet<string>,
  queuedResults: readonly ResourceMutationResultDto[],
): ReadonlyMap<string, string> {
  return buildSnapshotPathRemaps(
    authoritativeGraphPaths,
    queuedResults,
    (move) => move.kind === "event_graph" || move.kind === "function_graph",
  );
}

export function buildProjectSnapshotFilePathRemaps(
  authoritativeFilePaths: ReadonlySet<string>,
  queuedResults: readonly ResourceMutationResultDto[],
): ReadonlyMap<string, string> {
  return buildSnapshotPathRemaps(
    authoritativeFilePaths,
    queuedResults,
    (move) => move.kind === "chart" || move.kind === "mind" || move.kind === "doc",
  );
}

function remapDocuments(
  current: Readonly<Record<ResourceKey, DocumentState>>,
  plan: ProjectSnapshotPreparation,
): Record<ResourceKey, DocumentState> {
  const documents = structuredClone(current) as Record<ResourceKey, DocumentState>;
  const graphKind = new Map(nodeFileEntries(plan.index).map((graph) => [graph.path, graph.type]));
  for (const [from, to] of plan.pathRemaps) {
    const kind = graphKind.get(to);
    if (!kind) continue;
    const fromKey = resourceKey({ id: from, kind });
    const toKey = resourceKey({ id: to, kind });
    const source = documents[fromKey];
    if (!source) continue;
    documents[toKey] = { ...source, resourceKey: toKey };
    delete documents[fromKey];
  }
  for (const [from, to] of plan.filePathRemaps) {
    const kind =
      [...plan.index.minds, ...plan.index.docs].find((document) => document.path === to)?.kind ??
      "chart";
    const fromKey = resourceKey({ id: from, kind });
    const toKey = resourceKey({ id: to, kind });
    const source = documents[fromKey];
    if (!source) continue;
    documents[toKey] = { ...source, resourceKey: toKey };
    delete documents[fromKey];
  }
  return documents;
}

function remapResources(
  current: Readonly<Record<ResourceKey, ProjectResourceMeta>>,
  plan: ProjectSnapshotPreparation,
): Record<ResourceKey, ProjectResourceMeta> {
  const resources = structuredClone(current) as Record<ResourceKey, ProjectResourceMeta>;
  const graphByPath = new Map(nodeFileEntries(plan.index).map((graph) => [graph.path, graph]));
  for (const [from, to] of plan.pathRemaps) {
    const graph = graphByPath.get(to);
    if (!graph) continue;
    const fromKey = resourceKey({ id: from, kind: graph.type });
    const toKey = resourceKey({ id: to, kind: graph.type });
    const source = resources[fromKey];
    if (!source) continue;
    resources[toKey] = { ...source, id: to, uri: toKey, name: graph.name, kind: graph.type };
    delete resources[fromKey];
  }
  const fileByPath = new Map<
    string,
    { name: string; revision: number; kind: "chart" | "mind" | "doc" }
  >([
    ...plan.index.charts.map(
      (chart) =>
        [
          chart.chartPath,
          { name: chart.name, revision: chart.revision, kind: "chart" as const },
        ] as const,
    ),
    ...[...plan.index.minds, ...plan.index.docs].map(
      (document) => [document.path, document] as const,
    ),
  ]);
  for (const [from, to] of plan.filePathRemaps) {
    const file = fileByPath.get(to);
    if (!file) continue;
    const kind = file.kind;
    const fromKey = resourceKey({ id: from, kind });
    const toKey = resourceKey({ id: to, kind });
    const source = resources[fromKey];
    if (!source) continue;
    resources[toKey] = {
      ...source,
      id: to,
      uri: toKey,
      name: file.name,
      revision: file.revision,
      kind,
    };
    delete resources[fromKey];
  }
  return resources;
}

function applyDocumentPatches(
  documents: Record<ResourceKey, DocumentState>,
  patches: ReturnType<typeof prepareResourceProjectionSnapshot>["documentPatches"],
): void {
  for (const { key, patch } of patches) {
    const previous = documents[key];
    documents[key] = previous
      ? { ...previous, ...patch }
      : {
          resourceKey: key,
          loaded: true,
          dirty: patch.conflict ?? false,
          stale: patch.stale ?? false,
          missing: patch.missing ?? false,
          conflict: patch.conflict ?? false,
        };
  }
}

function prepareViewports(
  current: ReturnType<typeof useViewportStore.getState>["viewports"],
  pathRemaps: ReadonlyMap<string, string>,
  authoritativeGraphPaths: ReadonlySet<string>,
) {
  const viewports = structuredClone(current);
  for (const [from, to] of pathRemaps) {
    for (const key of Object.keys(viewports)) {
      const scope = parseViewportScopeKey(key);
      if (!scope || scope.graphPath !== from) continue;
      const destinationKey = viewportScopeKey({ ...scope, graphPath: to });
      if (viewports[destinationKey]) {
        throw new Error(`recovery viewport destination '${destinationKey}' already exists`);
      }
      viewports[destinationKey] = viewports[key];
      delete viewports[key];
    }
  }
  for (const key of Object.keys(viewports)) {
    const scope = parseViewportScopeKey(key);
    if (scope && !authoritativeGraphPaths.has(scope.graphPath)) delete viewports[key];
  }
  return viewports;
}

function databaseFromIndex(
  row: ProjectDatabaseIndexRow,
  current: DatabaseRecord | undefined,
): DatabaseRecord {
  const runtime: Partial<DatabaseRecord> = {};
  if (current?.columns !== undefined) runtime.columns = structuredClone(current.columns);
  if (current?.rowCount !== undefined) runtime.rowCount = current.rowCount;
  if (current?.columnCount !== undefined) runtime.columnCount = current.columnCount;
  runtime.loadFailed = current?.loadFailed === true;
  return {
    ...runtime,
    id: row.id,
    resourcePath: row.resourcePath,
    name: row.name ?? row.id,
    engine: structuredClone(row.engine),
    schemaVersion: row.schemaVersion,
    required: row.required,
  };
}

export function prepareProjectSnapshotCommit(
  plan: ProjectSnapshotPreparation,
): PreparedProjectSnapshot {
  const currentDatabases = useDatabaseStore.getState().databases;
  const databaseRows = plan.index.databases;
  const databases = Object.fromEntries(
    databaseRows.map((row) => [row.id, databaseFromIndex(row, currentDatabases[row.id])]),
  );
  const databaseRevisions = Object.fromEntries(databaseRows.map((row) => [row.id, row.revision]));
  const remappedDocuments = remapDocuments(useDocumentStateStore.getState().documents, plan);
  for (const key of plan.deletedResources) delete remappedDocuments[key];
  const chartState = useChartDocumentStore.getState();
  const authoritativeChartPaths = new Set(plan.index.charts.map((chart) => chart.chartPath));
  const remappedChartDocuments = structuredClone(chartState.documents);
  for (const [from, to] of plan.filePathRemaps) {
    const source = remappedChartDocuments[from];
    if (!source) continue;
    remappedChartDocuments[to] = source;
    delete remappedChartDocuments[from];
  }
  const chartDocuments = Object.fromEntries(
    Object.entries(remappedChartDocuments).filter(
      ([chartPath]) =>
        authoritativeChartPaths.has(chartPath) ||
        remappedDocuments[resourceKey({ id: chartPath, kind: "chart" })]?.dirty,
    ),
  );
  for (const [path, document] of plan.chartDocuments) {
    if (!remappedDocuments[resourceKey({ id: path, kind: "chart" })]?.dirty)
      chartDocuments[path] = document;
  }

  const graphMeta = Object.fromEntries(
    nodeFileEntries(plan.index).map((graph) => {
      const functionState =
        graph.type === "function_graph"
          ? {
              functionRevision: graph.functionEditorProjection.functionRevision,
              functionSignature: structuredClone(graph.functionSignature),
              functionInputs: structuredClone(graph.functionEditorProjection.inputs),
              functionOutputs: structuredClone(graph.functionEditorProjection.outputs),
            }
          : {};
      return [
        graph.path,
        {
          type: graph.type,
          ...functionState,
        },
      ];
    }),
  );

  const remappedResources = remapResources(useResourceStore.getState().resources, plan);
  for (const key of plan.deletedResources) delete remappedResources[key];
  const incoming = Object.values(
    buildProjectResourceState({
      eventGraphs: plan.index.eventGraphs,
      functionGraphs: plan.index.functionGraphs,
      charts: plan.index.charts,
      minds: plan.index.minds,
      docs: plan.index.docs,
      databases,
      loadedChartPaths: new Set(Object.keys(chartDocuments)),
    }).resources,
  );
  const { resources: projectedResources, documentPatches } = prepareResourceProjectionSnapshot(
    incoming,
    remappedResources,
    remappedDocuments,
  );
  const resources = Object.fromEntries(
    projectedResources.map((resource) => [resourceKey(resource), resource]),
  ) as Record<ResourceKey, ProjectResourceMeta>;
  applyDocumentPatches(remappedDocuments, documentPatches);
  const documents = remappedDocuments;
  const authoritativeGraphPaths = new Set(nodeFileEntries(plan.index).map((graph) => graph.path));
  const replacements = [...plan.graphSessions]
    .filter(([path, session]) => {
      const previousPath = [...plan.pathRemaps].find(([, to]) => to === path)?.[0] ?? path;
      return (
        !isGraphModified(previousPath) &&
        !isGraphSaving(previousPath) &&
        canAcceptGraphSession(useGraphProjectionStore.getState().sessions[previousPath], session) &&
        canAcceptGraphSession(useGraphProjectionStore.getState().sessions[path], session)
      );
    })
    .map(([graphPath, session]) => ({ graphPath, session }));
  const retainedGraphPaths = new Set([
    ...authoritativeGraphPaths,
    ...Object.keys(useGraphProjectionStore.getState().sessions).filter(
      (path) =>
        !plan.deletedResources.has(resourceKey({ id: path, kind: "event_graph" })) &&
        !plan.deletedResources.has(resourceKey({ id: path, kind: "function_graph" })) &&
        (isGraphModified(path) || isGraphSaving(path)),
    ),
  ]);
  const preparedGraphs = prepareGraphSessions(replacements, retainedGraphPaths);
  const refreshedKeys = [
    ...replacements.map(({ graphPath }) => {
      const kind = nodeFileEntries(plan.index).find((graph) => graph.path === graphPath)!.type;
      return resourceKey({ id: graphPath, kind });
    }),
    ...[...plan.chartDocuments.keys()].map((id) => resourceKey({ id, kind: "chart" })),
  ];
  for (const key of refreshedKeys) {
    if (documents[key]?.dirty) continue;
    if (documents[key])
      documents[key] = { ...documents[key], stale: false, missing: false, conflict: false };
    if (resources[key])
      resources[key] = {
        ...resources[key],
        loaded: true,
        hasStaleDocument: false,
        hasConflictDocument: false,
      };
  }
  const focused = useGraphSessionStore.getState().focusedSession;
  const remappedFocusedPath = focused
    ? (plan.pathRemaps.get(focused.graphPath) ?? focused.graphPath)
    : null;
  const focusedSession =
    focused && remappedFocusedPath && authoritativeGraphPaths.has(remappedFocusedPath)
      ? { ...focused, graphPath: remappedFocusedPath }
      : null;
  const viewports = prepareViewports(
    useViewportStore.getState().viewports,
    plan.pathRemaps,
    authoritativeGraphPaths,
  );

  return {
    ...plan,
    graphProjectionPlan: preparedGraphs,
    storeState: {
      resources,
      graphOrder: nodeFileEntries(plan.index).map((graph) => graph.path),
      documents,
      graphMeta,
      databases,
      databaseRevisions,
      chartDocuments,
      focusedSession,
      viewports,
    },
  };
}

export function commitPreparedProjectSnapshot(
  prepared: PreparedProjectSnapshot,
): void | Promise<void> {
  const moves = [
    ...[...prepared.pathRemaps].map(([from, to]) => ({ from, to })),
    ...[...prepared.filePathRemaps].map(([from, to]) => ({ from, to })),
  ];
  return commitEditorLayoutPublication(
    moves,
    prepared.storeState.resources,
    prepared.deletedResources,
    () => {
      assertCurrentProjectIdentity(prepared);
      const plan = prepareProjectSnapshotCommit(prepared);
      const fileProjections = [
        { kind: "mind", store: useMindProjectionStore, index: plan.index.minds },
        { kind: "doc", store: useDocProjectionStore, index: plan.index.docs },
      ] as const;
      for (const { store, index } of fileProjections) {
        for (const [from, to] of plan.filePathRemaps) {
          const entry = index.find((file) => file.path === to);
          const previous = store.getState().documents[from];
          if (entry && previous)
            remapDocumentInputs(from, to, previous.version, {
              sessionId: previous.version.sessionId,
              revision: entry.revision,
            });
        }
      }
      useDatabaseStore.setState({
        databases: plan.storeState.databases,
        revisions: plan.storeState.databaseRevisions,
      });
      useChartDocumentStore.setState({
        documents: plan.storeState.chartDocuments,
      });
      useDocumentStateStore.setState({ documents: plan.storeState.documents });
      useResourceStore.getState().setSnapshot({
        resources: Object.values(plan.storeState.resources),
        graphOrder: plan.storeState.graphOrder,
        publicationRevision: plan.publicationRevision,
      });
      useGraphMetaStore.setState({ graphs: plan.storeState.graphMeta });
      useSidebarStore.getState().publishPanels(plan.activityPanels);
      for (const { kind, store, index } of fileProjections) {
        const paths = new Set(index.map((file) => file.path));
        for (const path of Object.keys(store.getState().documents)) {
          const ref = { id: path, kind };
          if (
            !paths.has(path) &&
            (plan.deletedResources.has(resourceKey(ref)) ||
              (!shouldRetainResourceEditor(ref) && !hasPendingDocumentInput(path)))
          ) {
            releaseDocumentInputs(path);
            store.getState().remove(path);
          }
        }
      }
      const previousGraphs = useGraphProjectionStore.getState().sessions;
      useGraphProjectionStore.setState(plan.graphProjectionPlan.state);
      for (const path of new Set([
        ...plan.graphProjectionPlan.graphPaths,
        ...Object.keys(previousGraphs).filter(
          (path) => !plan.graphProjectionPlan.state.sessions[path],
        ),
      ])) {
        reconcileGraphResultQueries(path, previousGraphs[path]);
      }
      useGraphSessionStore.setState({ focusedSession: plan.storeState.focusedSession });
      useViewportStore.setState({ viewports: plan.storeState.viewports });
      for (const [from, to] of plan.pathRemaps) remapGraphNonViewportUiState(from, to);
      for (const [from, to] of plan.filePathRemaps) {
        remapFileNonViewportUiState(from, to);
        if (!plan.index.charts.some((chart) => chart.chartPath === to)) continue;
        invalidateChartPreviewCacheForMove(plan.projectInstanceId, from, to);
      }
      const focus = useEditorStore.getState().detailFocus;
      const focusedResource = detailResource(focus, plan.storeState.resources);
      if (
        detailResourceRef(focus) &&
        (!focusedResource || !shouldRetainResourceEditor(focusedResource))
      ) {
        useEditorStore.getState().clearDetailFocus();
      }
    },
    () => isCurrentProjectIdentity(prepared),
  );
}

/** Only explicit delete receipts authorize discarding dirty resources absent from the index. */
export function collectDeletedResourceKeys(
  index: ProjectIndexRow,
  receipts: readonly ResourceMutationResultDto[],
): ReadonlySet<ResourceKey> {
  const present = new Set([
    ...nodeFileEntries(index).map((file) => resourceKey({ id: file.path, kind: file.type })),
    ...index.charts.map((file) => resourceKey({ id: file.chartPath, kind: "chart" })),
    ...index.minds.map((file) => resourceKey({ id: file.path, kind: "mind" })),
    ...index.docs.map((file) => resourceKey({ id: file.path, kind: "doc" })),
  ]);
  const deleted = new Set<ResourceKey>();
  for (const receipt of receipts)
    for (const delta of receipt.deltas) {
      if (delta.payload.kind !== "resource_lifecycle") continue;
      const { before, after } = delta.payload.patch;
      if (!before || after) continue;
      const key = resourceKey({ id: before.path, kind: before.kind });
      if (!present.has(key)) deleted.add(key);
    }
  return deleted;
}
