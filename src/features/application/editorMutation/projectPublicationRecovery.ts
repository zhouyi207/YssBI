import type { ResourceMutationResultDto } from "@/shared/types/domain/editorMutation";
import type { ProjectDatabaseIndexRow, ProjectIndexRow } from "@/shared/types/domain/project";
import { isProjectDatabaseIndexRow } from "@/services/project/projectService";
import type { DatabaseRecord } from "@/shared/types/domain/database";
import { displayNameFromEngine } from "@/features/application/dataManagement/databaseRecords";
import type {
  PreparedProjectRecovery,
  ProjectRecoveryPreparation,
} from "./projectPublicationCoordinator";
import { useDatabaseStore, useGraphMetaStore, useVariableStore } from "@/features/core/dataStore";
import {
  commitPreparedGraphProjectionReplacements,
  prepareGraphProjectionReplacements,
  useGraphDataStore,
} from "@/features/core/dataStore/graphDataStore";
import { useChartDocumentStore } from "@/features/core/chart/chartDocumentStore";
import {
  buildGraphResourceMeta,
  reconcileResourceSnapshot,
  resourceKey,
  useDocumentStateStore,
  useResourceStore,
  type DocumentState,
  type ProjectResourceMeta,
  type ResourceKey,
} from "@/features/core/resource";
import {
  applyVariableCatalogFromIndex,
  variableCatalogToResourceMetas,
  variableRevisionsFromIndex,
} from "@/features/core/variable/variableCatalog";
import { useHistoryStore } from "@/features/core/history";
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

function publicationPaths(result: ResourceMutationResultDto): string[] {
  const statusPaths =
    result.projectionStatus.status === "complete"
      ? result.projectionStatus.expectedGraphPaths
      : result.projectionStatus.invalidatedGraphPaths;
  const lifecyclePaths = result.deltas.flatMap((delta) => {
    if (delta.resource.kind !== "graph" || delta.payload.kind !== "resource_lifecycle") return [];
    const { before, after } = delta.payload.patch;
    return [before?.path, after?.path].filter((path): path is string => path != null);
  });
  return [
    ...statusPaths,
    ...result.moves.filter((move) => move.kind !== "chart").map((move) => move.to),
    ...lifecyclePaths,
  ];
}

function validFunctionSignature(value: unknown): boolean {
  if (typeof value !== "object" || value === null) return false;
  const signature = value as { parameters?: unknown; return_type?: unknown };
  return (
    Array.isArray(signature.parameters) &&
    signature.parameters.every((parameter) => {
      if (typeof parameter !== "object" || parameter === null) return false;
      const row = parameter as Record<string, unknown>;
      return (
        typeof row.id === "string" &&
        typeof row.name === "string" &&
        typeof row.type_name === "string"
      );
    }) &&
    (signature.return_type === null || typeof signature.return_type === "string")
  );
}

export function validateProjectRecoveryIndex(
  index: ProjectIndexRow,
  projectInstanceId: string,
): string | undefined {
  if (index.projectInstanceId !== projectInstanceId) return "recovery project identity is stale";
  if (!Number.isSafeInteger(index.publicationRevision) || index.publicationRevision < 0) {
    return "recovery publication revision is malformed";
  }
  if (typeof index.history?.canUndo !== "boolean" || typeof index.history?.canRedo !== "boolean") {
    return "recovery history is malformed";
  }
  if (
    !Array.isArray(index.graphs) ||
    index.graphs.some(
      (graph) =>
        typeof graph.path !== "string" ||
        typeof graph.name !== "string" ||
        (graph.type !== "event" && graph.type !== "function") ||
        (graph.type === "function" &&
          (!Number.isSafeInteger(graph.functionRevision) ||
            (graph.functionRevision as number) < 0 ||
            !validFunctionSignature(graph.functionSignature))),
    )
  ) {
    return "recovery graph metadata is malformed";
  }
  if (new Set(index.graphs.map((graph) => graph.path)).size !== index.graphs.length) {
    return "recovery graph metadata contains duplicate paths";
  }
  if (!Array.isArray(index.variables) || !Array.isArray(index.charts)) {
    return "recovery resource index is incomplete";
  }
  if (
    !Array.isArray(index.databases) ||
    index.databases.some((database) => !isProjectDatabaseIndexRow(database)) ||
    new Set(index.databases.map((database) => database.id)).size !== index.databases.length
  ) {
    return "recovery database metadata is malformed";
  }
  if (
    index.charts.some(
      (chart) =>
        typeof chart.chartPath !== "string" ||
        !chart.chartPath ||
        typeof chart.name !== "string" ||
        typeof chart.databaseId !== "string" ||
        !Number.isSafeInteger(chart.revision) ||
        chart.revision < 0 ||
        (chart.chartType !== "histogram" &&
          chart.chartType !== "scatter" &&
          chart.chartType !== "line"),
    ) ||
    new Set(index.charts.map((chart) => chart.chartPath)).size !== index.charts.length
  ) {
    return "recovery chart metadata is malformed";
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

function buildRecoveryPathRemaps(
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

export function buildProjectRecoveryPathRemaps(
  authoritativeGraphPaths: ReadonlySet<string>,
  queuedResults: readonly ResourceMutationResultDto[],
): ReadonlyMap<string, string> {
  return buildRecoveryPathRemaps(
    authoritativeGraphPaths,
    queuedResults,
    (move) => move.kind === "event" || move.kind === "function",
  );
}

export function buildProjectRecoveryChartPathRemaps(
  authoritativeChartPaths: ReadonlySet<string>,
  queuedResults: readonly ResourceMutationResultDto[],
): ReadonlyMap<string, string> {
  return buildRecoveryPathRemaps(
    authoritativeChartPaths,
    queuedResults,
    (move) => move.kind === "chart",
  );
}

export function collectProjectRecoveryGraphPaths(
  index: ProjectIndexRow,
  graphPathsLoadedAtStart: ReadonlySet<string>,
  queuedResults: readonly ResourceMutationResultDto[],
): ReadonlySet<string> {
  const authoritative = new Set(index.graphs.map((graph) => graph.path));
  const pathRemaps = buildProjectRecoveryPathRemaps(authoritative, queuedResults);
  const rewrite = (path: string): string => pathRemaps.get(path) ?? path;
  const candidates = new Set<string>();
  for (const path of graphPathsLoadedAtStart) candidates.add(rewrite(path));
  for (const result of queuedResults) {
    for (const path of publicationPaths(result)) candidates.add(rewrite(path));
  }

  return new Set([...candidates].filter((path) => authoritative.has(path)));
}

function recoveryResources(
  index: ProjectIndexRow,
  variables: ReturnType<typeof applyVariableCatalogFromIndex>,
  chartDocuments: Readonly<Record<string, unknown>>,
  databases: Readonly<Record<string, { name?: unknown }>>,
): ProjectResourceMeta[] {
  const resources: ProjectResourceMeta[] = index.graphs.map((graph) =>
    buildGraphResourceMeta(graph.type, graph.path, graph.name, { revision: graph.revision }),
  );
  resources.push(
    ...index.charts.map((chart) => ({
      id: chart.chartPath,
      kind: "chart" as const,
      name: chart.name,
      uri: `yssbi://chart/${chart.chartPath}`,
      revision: chart.revision,
      exists: true,
      loaded: Boolean(chartDocuments[chart.chartPath]),
      hasDirtyDocument: false,
      hasStaleDocument: false,
      hasConflictDocument: false,
    })),
  );
  resources.push(...variableCatalogToResourceMetas(variables));
  for (const [id, database] of Object.entries(databases)) {
    resources.push({
      id,
      kind: "database",
      name: typeof database.name === "string" ? database.name : id,
      uri: `yssbi://database/${id}`,
      exists: true,
      loaded: true,
      hasDirtyDocument: false,
      hasStaleDocument: false,
      hasConflictDocument: false,
    });
  }
  return resources;
}

function remapDocuments(
  current: Readonly<Record<ResourceKey, DocumentState>>,
  plan: ProjectRecoveryPreparation,
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
  for (const [from, to] of plan.chartPathRemaps ?? []) {
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
  plan: ProjectRecoveryPreparation,
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
  for (const [from, to] of plan.chartPathRemaps ?? []) {
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
  patches: ReturnType<typeof reconcileResourceSnapshot>["documentPatches"],
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
    name: row.name ?? displayNameFromEngine(row.engine) ?? row.id,
    engine: structuredClone(row.engine),
    schemaVersion: row.schemaVersion,
    required: row.required,
  };
}

export function prepareProjectRecoveryCommit(
  plan: ProjectRecoveryPreparation,
): PreparedProjectRecovery {
  const variables = applyVariableCatalogFromIndex(plan.index.variables);
  const variableRevisions = variableRevisionsFromIndex(plan.index.variables);
  const currentDatabases = useDatabaseStore.getState().databases;
  const databaseRows = plan.index.databases;
  const databases = Object.fromEntries(
    databaseRows.map((row) => [row.id, databaseFromIndex(row, currentDatabases[row.id])]),
  );
  const databaseRevisions = Object.fromEntries(databaseRows.map((row) => [row.id, row.revision]));
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
  for (const [from, to] of plan.chartPathRemaps ?? []) {
    const source = remappedChartDocuments[from];
    if (!source) continue;
    remappedChartDocuments[to] = source;
    delete remappedChartDocuments[from];
  }
  const chartDocuments = Object.fromEntries(
    Object.entries(remappedChartDocuments).filter(([chartPath]) =>
      authoritativeChartPaths.has(chartPath),
    ),
  );

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

  const remappedDocuments = remapDocuments(useDocumentStateStore.getState().documents, plan);
  const remappedResources = remapResources(useResourceStore.getState().resources, plan);
  const incoming = recoveryResources(plan.index, variables, chartDocuments, databases);
  const { resources: reconciledResources, documentPatches } = reconcileResourceSnapshot(
    incoming,
    remappedResources,
    remappedDocuments,
  );
  const authoritativeKeys = new Set(incoming.map((resource) => resourceKey(resource)));
  const resources = Object.fromEntries(
    reconciledResources
      .filter((resource) => authoritativeKeys.has(resourceKey(resource)))
      .map((resource) => [resourceKey(resource), resource]),
  ) as Record<ResourceKey, ProjectResourceMeta>;
  applyDocumentPatches(
    remappedDocuments,
    documentPatches.filter(({ key }) => authoritativeKeys.has(key)),
  );
  const previousPathOwnedKeys = new Set(
    Object.values(remappedResources)
      .filter(
        (resource) =>
          resource.kind === "event" || resource.kind === "function" || resource.kind === "chart",
      )
      .map((resource) => resourceKey(resource)),
  );
  const documents = Object.fromEntries(
    Object.entries(remappedDocuments).filter(
      ([key]) =>
        !previousPathOwnedKeys.has(key as ResourceKey) || authoritativeKeys.has(key as ResourceKey),
    ),
  ) as Record<ResourceKey, DocumentState>;

  const authoritativeGraphPaths = new Set(plan.index.graphs.map((graph) => graph.path));
  const replacements = [...plan.projections].map(([graphPath, projection]) => ({
    graphPath,
    projection,
  }));
  const loadedAtStartTerminals = new Set(
    [...plan.graphPathsLoadedAtStart].map((path) => plan.pathRemaps.get(path) ?? path),
  );
  const concurrentGraphEntities = Object.fromEntries(
    Object.entries(useGraphDataStore.getState().graphEntities).filter(
      ([path]) => authoritativeGraphPaths.has(path) && !loadedAtStartTerminals.has(path),
    ),
  );
  const preparedGraphs = prepareGraphProjectionReplacements(replacements, concurrentGraphEntities);
  if (!preparedGraphs.prepared) {
    throw new Error(`recovery projection preparation failed for '${preparedGraphs.graphPath}'`);
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
      variables,
      variableRevisions,
      chartIndex,
      chartDocuments,
      focusedSession,
      viewports,
    },
    history: { ...plan.index.history },
  };
}

export function commitPreparedProjectRecovery(plan: PreparedProjectRecovery): void | Promise<void> {
  const moves = [
    ...[...plan.pathRemaps].map(([from, to]) => ({ from, to })),
    ...[...(plan.chartPathRemaps ?? [])].map(([from, to]) => ({ from, to })),
  ];
  return commitEditorDockviewPublication(moves, plan.storeState.resources, () => {
    useDatabaseStore.setState({
      databases: plan.storeState.databases,
      revisions: plan.storeState.databaseRevisions,
    });
    useVariableStore.setState({
      variables: plan.storeState.variables,
      revisions: plan.storeState.variableRevisions,
    });
    useChartDocumentStore.setState({
      index: plan.storeState.chartIndex,
      documents: plan.storeState.chartDocuments,
    });
    useDocumentStateStore.setState({ documents: plan.storeState.documents });
    useResourceStore.setState({
      resources: plan.storeState.resources,
      graphOrder: plan.storeState.graphOrder,
    });
    useGraphMetaStore.setState({ graphs: plan.storeState.graphMeta });
    commitPreparedGraphProjectionReplacements(plan.graphProjectionPlan);
    useGraphSessionStore.setState({ focusedSession: plan.storeState.focusedSession });
    useViewportStore.setState({ viewports: plan.storeState.viewports });
    useHistoryStore.setState({
      canUndo: plan.history.canUndo,
      canRedo: plan.history.canRedo,
    });
    for (const [from, to] of plan.pathRemaps) remapGraphNonViewportUiState(from, to);
    for (const [from, to] of plan.chartPathRemaps ?? []) {
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
  });
}
