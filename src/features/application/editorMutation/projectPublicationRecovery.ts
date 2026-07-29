import type { ResourceMutationResultDto } from '@/shared/types/dto/editorMutation';
import type { ProjectIndexRow } from '@/services/project/projectService';
import type {
  PreparedProjectRecovery,
  ProjectRecoveryPreparation,
} from './projectPublicationCoordinator';
import { useDatabaseStore, useGraphMetaStore, useVariableStore } from '@/features/core/dataStore';
import {
  commitPreparedGraphProjectionReplacements,
  prepareGraphProjectionReplacements,
} from '@/features/core/dataStore/graphDataStore';
import { useWorksheetStore } from '@/features/core/worksheet/worksheetStore';
import {
  buildGraphResourceMeta,
  reconcileResourceSnapshot,
  resourceKey,
  useDocumentStateStore,
  useResourceStore,
  type DocumentState,
  type ProjectResourceMeta,
  type ResourceKey,
} from '@/features/core/resource';
import {
  applyVariableCatalogFromIndex,
  variableCatalogToResourceMetas,
  variableRevisionsFromIndex,
} from '@/features/core/variable/variableCatalog';
import { functionSignaturePins } from '@/features/application/graphDocument/functionSignatureSync';
import { useHistoryStore } from '@/features/core/history';
import { useGraphSessionStore } from '@/features/core/graphSession/graphSessionStore';
import { useEditorTabStore, type EditorTabMemento } from '@/features/core/layout/editorTabStore';
import { useViewportStore } from '@/features/core/viewport';
import { parseViewportScopeKey, viewportScopeKey } from '@/features/core/viewport/viewportScope';

function publicationPaths(result: ResourceMutationResultDto): string[] {
  const statusPaths = result.projectionStatus.status === 'complete'
    ? result.projectionStatus.expectedGraphPaths
    : result.projectionStatus.invalidatedGraphPaths;
  const lifecyclePaths = result.deltas.flatMap((delta) => {
    if (delta.payload.kind !== 'graph_resource_lifecycle') return [];
    const { before, after } = delta.payload.patch;
    return [before?.path, after?.path].filter((path): path is string => path != null);
  });
  return [...statusPaths, ...result.moves.map((move) => move.to), ...lifecyclePaths];
}

function validFunctionSignature(value: unknown): boolean {
  if (typeof value !== 'object' || value === null) return false;
  const signature = value as { parameters?: unknown; return_type?: unknown };
  return Array.isArray(signature.parameters)
    && signature.parameters.every((parameter) => {
      if (typeof parameter !== 'object' || parameter === null) return false;
      const row = parameter as Record<string, unknown>;
      return typeof row.id === 'string'
        && typeof row.name === 'string'
        && typeof row.type_name === 'string';
    })
    && (signature.return_type === null || typeof signature.return_type === 'string');
}

export function validateProjectRecoveryIndex(
  index: ProjectIndexRow,
  projectInstanceId: string,
): string | undefined {
  if (index.projectInstanceId !== projectInstanceId) return 'recovery project identity is stale';
  if (!Number.isSafeInteger(index.publicationRevision) || index.publicationRevision < 0) {
    return 'recovery publication revision is malformed';
  }
  if (typeof index.history?.canUndo !== 'boolean' || typeof index.history?.canRedo !== 'boolean') {
    return 'recovery history is malformed';
  }
  if (!Array.isArray(index.graphs) || index.graphs.some((graph) =>
    typeof graph.path !== 'string'
    || typeof graph.name !== 'string'
    || (graph.type !== 'event' && graph.type !== 'function')
    || (graph.type === 'function'
      && (!Number.isSafeInteger(graph.functionRevision)
        || (graph.functionRevision as number) < 0
        || !validFunctionSignature(graph.functionSignature))))) {
    return 'recovery graph metadata is malformed';
  }
  if (new Set(index.graphs.map((graph) => graph.path)).size !== index.graphs.length) {
    return 'recovery graph metadata contains duplicate paths';
  }
  if (!Array.isArray(index.variables) || !Array.isArray(index.worksheets)) {
    return 'recovery resource index is incomplete';
  }
  if (index.worksheets.some((worksheet) =>
    typeof worksheet.id !== 'string'
    || !worksheet.id
    || typeof worksheet.name !== 'string'
    || typeof worksheet.databaseId !== 'string'
    || (worksheet.chartType !== 'histogram'
      && worksheet.chartType !== 'scatter'
      && worksheet.chartType !== 'line'))
    || new Set(index.worksheets.map((worksheet) => worksheet.id)).size !== index.worksheets.length) {
    return 'recovery worksheet metadata is malformed';
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

export function buildProjectRecoveryPathRemaps(
  authoritativeGraphPaths: ReadonlySet<string>,
  queuedResults: readonly ResourceMutationResultDto[],
): ReadonlyMap<string, string> {
  const movesBySource = new Map<string, Set<string>>();
  for (const result of queuedResults) {
    for (const move of result.moves) {
      const destinations = movesBySource.get(move.from) ?? new Set<string>();
      destinations.add(move.to);
      movesBySource.set(move.from, destinations);
    }
  }

  const destinationOwners = new Map<string, string>();
  const pathRemaps = new Map<string, string>();
  for (const source of movesBySource.keys()) {
    if (authoritativeGraphPaths.has(source)) continue;
    const terminals = [...authoritativeTerminals(authoritativeGraphPaths, movesBySource, source)];
    if (terminals.length > 1) {
      throw new Error(`conflicting recovery move source '${source}'`);
    }
    const terminal = terminals[0];
    if (!terminal) continue;
    const destinationOwner = destinationOwners.get(terminal);
    if (destinationOwner
      && !canReach(movesBySource, destinationOwner, source)
      && !canReach(movesBySource, source, destinationOwner)) {
      throw new Error(`conflicting recovery move destination '${terminal}'`);
    }
    pathRemaps.set(source, terminal);
    destinationOwners.set(terminal, destinationOwner ?? source);
  }
  return pathRemaps;
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
  worksheetDocuments: Readonly<Record<string, unknown>>,
  databases: Readonly<Record<string, { name?: unknown }>>,
): ProjectResourceMeta[] {
  const resources: ProjectResourceMeta[] = index.graphs.map((graph) =>
    buildGraphResourceMeta(graph.type, graph.path, graph.name));
  resources.push(...(index.worksheets ?? []).map((worksheet) => ({
    id: worksheet.id,
    kind: 'worksheet' as const,
    name: worksheet.name,
    uri: `yssbi://worksheet/${worksheet.id}`,
    exists: true,
    loaded: Boolean(worksheetDocuments[worksheet.id]),
    hasDirtyDocument: false,
    hasStaleDocument: false,
    hasConflictDocument: false,
  })));
  resources.push(...variableCatalogToResourceMetas(variables));
  for (const [id, database] of Object.entries(databases)) {
    resources.push({
      id,
      kind: 'database',
      name: typeof database.name === 'string' ? database.name : id,
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
    if (documents[fromKey] && documents[toKey]) {
      throw new Error(`recovery document destination '${to}' already exists`);
    }
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
    if (resources[fromKey] && resources[toKey]) {
      throw new Error(`recovery resource destination '${to}' already exists`);
    }
    const source = resources[fromKey];
    if (!source) continue;
    resources[toKey] = { ...source, id: to, uri: toKey, name: graph.name, kind: graph.type };
    delete resources[fromKey];
  }
  return resources;
}

function applyDocumentPatches(
  documents: Record<ResourceKey, DocumentState>,
  patches: ReturnType<typeof reconcileResourceSnapshot>['documentPatches'],
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

function remapTabs(
  current: EditorTabMemento,
  pathRemaps: ReadonlyMap<string, string>,
): EditorTabMemento {
  const tabs = structuredClone(current);
  for (const [from, to] of pathRemaps) {
    const source = tabs.registry[from];
    if (source && tabs.registry[to]) throw new Error(`recovery tab destination '${to}' already exists`);
    if (source) {
      tabs.registry[to] = { ...source, id: to };
      delete tabs.registry[from];
    }
    for (const placement of Object.values(tabs.placements)) {
      placement.tabIds = placement.tabIds.map((id) => id === from ? to : id);
      placement.selectedTabIds = placement.selectedTabIds.map((id) => id === from ? to : id);
      if (placement.activeTabId === from) placement.activeTabId = to;
    }
  }
  return tabs;
}

function reconcileTabs(
  tabs: EditorTabMemento,
  resources: Readonly<Record<ResourceKey, ProjectResourceMeta>>,
): EditorTabMemento {
  for (const [tabId, tab] of Object.entries(tabs.registry)) {
    if ((tab.type === 'event' || tab.type === 'function' || tab.type === 'worksheet')
      && !resources[resourceKey({ id: tabId, kind: tab.type })]) {
      delete tabs.registry[tabId];
    }
  }
  for (const [groupId, placement] of Object.entries(tabs.placements)) {
    placement.tabIds = placement.tabIds.filter((tabId) => Boolean(tabs.registry[tabId]));
    placement.selectedTabIds = placement.selectedTabIds.filter((tabId) =>
      placement.tabIds.includes(tabId));
    if (!placement.activeTabId || !placement.tabIds.includes(placement.activeTabId)) {
      placement.activeTabId = placement.tabIds[placement.tabIds.length - 1] ?? null;
    }
    if (placement.tabIds.length === 0) delete tabs.placements[groupId];
  }
  return tabs;
}

function prepareViewports(
  current: ReturnType<typeof useViewportStore.getState>['viewports'],
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

export function prepareProjectRecoveryCommit(
  plan: ProjectRecoveryPreparation,
): PreparedProjectRecovery {
  const variables = applyVariableCatalogFromIndex(plan.index.variables);
  const variableRevisions = variableRevisionsFromIndex(plan.index.variables);
  const worksheetState = useWorksheetStore.getState();
  const worksheetIndex = (plan.index.worksheets ?? []).map((worksheet) => ({
    id: worksheet.id,
    name: worksheet.name,
    databaseId: worksheet.databaseId,
    chartType: worksheet.chartType as import('@/shared/types/domain/worksheet').WorksheetChartType,
  }));
  const authoritativeWorksheetIds = new Set(worksheetIndex.map((worksheet) => worksheet.id));
  const worksheetDocuments = Object.fromEntries(
    Object.entries(worksheetState.documents).filter(([id]) => authoritativeWorksheetIds.has(id)),
  );

  const graphMeta = Object.fromEntries(plan.index.graphs.map((graph) => {
    const functionState = graph.type === 'function' && graph.functionSignature
      ? {
          functionRevision: graph.functionRevision,
          functionSignature: structuredClone(graph.functionSignature),
          ...functionSignaturePins(graph.functionSignature),
        }
      : {};
    return [graph.path, {
      path: graph.path,
      name: graph.name,
      type: graph.type,
      ...functionState,
    }];
  }));

  const remappedDocuments = remapDocuments(useDocumentStateStore.getState().documents, plan);
  const remappedResources = remapResources(useResourceStore.getState().resources, plan);
  const incoming = recoveryResources(
    plan.index,
    variables,
    worksheetDocuments,
    useDatabaseStore.getState().databases,
  );
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
      .filter((resource) => resource.kind === 'event'
        || resource.kind === 'function'
        || resource.kind === 'worksheet')
      .map((resource) => resourceKey(resource)),
  );
  const documents = Object.fromEntries(Object.entries(remappedDocuments).filter(([key]) =>
    !previousPathOwnedKeys.has(key as ResourceKey) || authoritativeKeys.has(key as ResourceKey),
  )) as Record<ResourceKey, DocumentState>;

  const replacements = [...plan.projections].map(([graphPath, projection]) => ({
    graphPath,
    projection,
  }));
  const preparedGraphs = prepareGraphProjectionReplacements(replacements, {});
  if (!preparedGraphs.prepared) {
    throw new Error(`recovery projection preparation failed for '${preparedGraphs.graphPath}'`);
  }

  const authoritativeGraphPaths = new Set(plan.index.graphs.map((graph) => graph.path));
  const focused = useGraphSessionStore.getState().focusedSession;
  const remappedFocusedPath = focused
    ? plan.pathRemaps.get(focused.graphPath) ?? focused.graphPath
    : null;
  const focusedSession = focused && remappedFocusedPath && authoritativeGraphPaths.has(remappedFocusedPath)
    ? { ...focused, graphPath: remappedFocusedPath }
    : null;
  const tabs = reconcileTabs(
    remapTabs(useEditorTabStore.getState().snapshotMemento(), plan.pathRemaps),
    resources,
  );
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
      variables,
      variableRevisions,
      worksheetIndex,
      worksheetDocuments,
      tabs,
      focusedSession,
      viewports,
    },
    history: { ...plan.index.history },
  };
}

export function commitPreparedProjectRecovery(plan: PreparedProjectRecovery): void {
  useVariableStore.setState({
    variables: plan.storeState.variables,
    revisions: plan.storeState.variableRevisions,
  });
  useWorksheetStore.setState({
    index: plan.storeState.worksheetIndex,
    documents: plan.storeState.worksheetDocuments,
  });
  useDocumentStateStore.setState({ documents: plan.storeState.documents });
  useResourceStore.setState({
    resources: plan.storeState.resources,
    graphOrder: plan.storeState.graphOrder,
  });
  useGraphMetaStore.setState({ graphs: plan.storeState.graphMeta });
  commitPreparedGraphProjectionReplacements(plan.graphProjectionPlan);
  useGraphSessionStore.setState({ focusedSession: plan.storeState.focusedSession });
  useEditorTabStore.setState(plan.storeState.tabs);
  useViewportStore.setState({ viewports: plan.storeState.viewports });
  useHistoryStore.setState({
    canUndo: plan.history.canUndo,
    canRedo: plan.history.canRedo,
  });
}
