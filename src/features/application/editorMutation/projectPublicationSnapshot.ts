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
  prepareGraphProjectionReplacements,
  useGraphProjectionStore,
} from "@/features/core/dataStore/graphProjectionStore";
import {
  isGraphDraftDirty,
  isGraphDraftSaving,
  useGraphDraftStore,
} from "@/features/core/graphDraft";
import {
  assertCurrentProjectIdentity,
  isCurrentProjectIdentity,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { useChartDocumentStore } from "@/features/core/chart/chartDocumentStore";
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
  remapChartNonViewportUiState,
} from "@/features/application/editor/cascadeGraphPathReferences";
import { invalidateChartPreviewCacheForMove } from "@/services/chart/chartPreviewCache";
import { commitEditorDockviewPublication } from "./editorDockviewPublicationCommit";
import { buildProjectResourceState } from "@/features/application/project/authoritativeProjectLoadPlan";

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
    (move) => move.kind === "event" || move.kind === "function",
  );
}

export function buildProjectSnapshotChartPathRemaps(
  authoritativeChartPaths: ReadonlySet<string>,
  queuedResults: readonly ResourceMutationResultDto[],
): ReadonlyMap<string, string> {
  return buildSnapshotPathRemaps(
    authoritativeChartPaths,
    queuedResults,
    (move) => move.kind === "chart",
  );
}

function remapDocuments(
  current: Readonly<Record<ResourceKey, DocumentState>>,
  plan: ProjectSnapshotPreparation,
): Record<ResourceKey, DocumentState> {
  const documents = structuredClone(current) as Record<ResourceKey, DocumentState>;
  const graphKind = new Map(plan.index.graphs.map((graph) => [graph.path, graph.type]));
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
  for (const [from, to] of plan.chartPathRemaps) {
    const fromKey = resourceKey({ id: from, kind: "chart" });
    const toKey = resourceKey({ id: to, kind: "chart" });
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
  const graphByPath = new Map(plan.index.graphs.map((graph) => [graph.path, graph]));
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
  const chartByPath = new Map(plan.index.charts.map((chart) => [chart.chartPath, chart]));
  for (const [from, to] of plan.chartPathRemaps) {
    const chart = chartByPath.get(to);
    if (!chart) continue;
    const fromKey = resourceKey({ id: from, kind: "chart" });
    const toKey = resourceKey({ id: to, kind: "chart" });
    const source = resources[fromKey];
    if (!source) continue;
    resources[toKey] = {
      ...source,
      id: to,
      uri: toKey,
      name: chart.name,
      revision: chart.revision,
      kind: "chart",
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
          version: 0,
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
  const chartState = useChartDocumentStore.getState();
  const chartIndex = plan.index.charts.map((chart) => ({
    chartPath: chart.chartPath,
    name: chart.name,
    databaseId: chart.databaseId,
    chartType: chart.chartType as import("@/shared/types/domain/chart").ChartType,
    revision: chart.revision,
  }));
  const authoritativeChartPaths = new Set(chartIndex.map((chart) => chart.chartPath));
  const remappedChartDocuments = structuredClone(chartState.documents);
  for (const [from, to] of plan.chartPathRemaps) {
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
    plan.index.graphs.map((graph) => {
      const functionState =
        graph.type === "function"
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
          path: graph.path,
          name: graph.name,
          type: graph.type,
          ...functionState,
        },
      ];
    }),
  );

  const remappedResources = remapResources(useResourceStore.getState().resources, plan);
  const incoming = Object.values(
    buildProjectResourceState({
      graphs: plan.index.graphs,
      charts: plan.index.charts,
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
  const authoritativeGraphPaths = new Set(plan.index.graphs.map((graph) => graph.path));
  const replacements = [...plan.graphSessions]
    .filter(([path]) => {
      const previousPath = [...plan.pathRemaps].find(([, to]) => to === path)?.[0] ?? path;
      return !isGraphDraftDirty(previousPath) && !isGraphDraftSaving(previousPath);
    })
    .map(([graphPath, session]) => ({ graphPath, projection: session.projection }));
  const retainedGraphEntities = Object.fromEntries(
    Object.entries(useGraphProjectionStore.getState().graphEntities).filter(
      ([path]) =>
        authoritativeGraphPaths.has(path) || isGraphDraftDirty(path) || isGraphDraftSaving(path),
    ),
  );
  const preparedGraphs = prepareGraphProjectionReplacements(replacements, retainedGraphEntities);
  const refreshedKeys = [
    ...replacements.map(({ graphPath }) => {
      const kind = plan.index.graphs.find((graph) => graph.path === graphPath)!.type;
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
  if (!preparedGraphs.prepared) {
    throw new Error(`snapshot projection preparation failed for '${preparedGraphs.graphPath}'`);
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
    graphProjectionPlan: preparedGraphs.plan,
    storeState: {
      resources,
      graphOrder: plan.index.graphs.map((graph) => graph.path),
      documents,
      graphMeta,
      databases,
      databaseRevisions,
      chartIndex,
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
    ...[...prepared.chartPathRemaps].map(([from, to]) => ({ from, to })),
  ];
  return commitEditorDockviewPublication(
    moves,
    prepared.storeState.resources,
    () => {
      assertCurrentProjectIdentity(prepared);
      const plan = prepareProjectSnapshotCommit(prepared);
      useDatabaseStore.setState({
        databases: plan.storeState.databases,
        revisions: plan.storeState.databaseRevisions,
      });
      useChartDocumentStore.setState({
        index: plan.storeState.chartIndex,
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
      useGraphProjectionStore.setState({ graphEntities: plan.graphProjectionPlan.graphEntities });
      for (const path of plan.graphProjectionPlan.graphPaths) {
        const session = plan.graphSessions.get(path);
        if (session) useGraphDraftStore.getState().hydrate(path, session);
      }
      useGraphSessionStore.setState({ focusedSession: plan.storeState.focusedSession });
      useViewportStore.setState({ viewports: plan.storeState.viewports });
      for (const [from, to] of plan.pathRemaps) remapGraphNonViewportUiState(from, to);
      for (const [from, to] of plan.chartPathRemaps) {
        remapChartNonViewportUiState(from, to);
        invalidateChartPreviewCacheForMove(plan.projectInstanceId, from, to);
      }
      const detailFocus = useEditorStore.getState().detailFocus;
      if (
        detailFocus?.kind === "chart" &&
        !plan.index.charts.some((chart) => chart.chartPath === detailFocus.chartPath)
      ) {
        useEditorStore.getState().clearDetailFocus();
      }
    },
    () => isCurrentProjectIdentity(prepared),
  );
}
