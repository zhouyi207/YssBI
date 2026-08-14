import {
  commitPreparedGraphProjectionReplacements,
  prepareGraphProjectionReplacements,
  useGraphDataStore,
  type GraphEntityBucket,
} from '@/features/core/dataStore/graphDataStore';
import { useGraphMetaStore } from '@/features/core/dataStore/graphMetaStore';
import { useDatabaseStore } from '@/features/core/dataStore/databaseStore';
import { useVariableStore } from '@/features/core/dataStore/variableStore';

import { validateResourceMutationWireResult } from '@/shared/types/dto/resourceMutationResultValidator';
export { validateResourceMutationWireResult } from '@/shared/types/dto/resourceMutationResultValidator';
import { toProjectionEntities } from '@/features/domain/editorProjection';
import { isGraphResourcePath } from '@/shared/types/dto/editorProjectionGuards';
import { inferGraphResourceKind } from '@/shared/types/domain/graphResourcePath';
import {
  normalizeDatabaseRecord,
  type DatabaseDocumentDto,
  type DatabaseRecord,
} from '@/shared/types/dto/database';
import { normalizeVariableFromBackend } from '@/shared/types/dto/variable';
import { variableCatalogToResourceMetas } from '@/features/core/variable/variableCatalog';
import type {
  GraphProjectionReplacementDto,
  ResourceDeltaDto,
  ResourceMutationResultDto,
  VariableDocumentPatchDto,
} from '@/shared/types/dto/editorMutation';
import type { Variable } from '@/shared/types/domain/variable';
import type { WorksheetDocument, WorksheetIndexEntry } from '@/shared/types/domain/worksheet';
import {
  installFunctionEditorProjection,
} from '@/features/application/graphDocument/functionSignatureSync';
import {
  remapGraphNonViewportUiState,
  remapWorksheetNonViewportUiState,
} from '@/features/application/editor/cascadeGraphPathReferences';
import { invalidateWorksheetPreviewCacheForMove } from '@/services/worksheet/worksheetPreviewCache';

import type {
  PreparedFunctionDeltaInstall,
  PreparedProjectPublication,
  PreparePublicationContext,
  PreparedVariableDeltaInstall,
} from './projectPublicationCoordinator';
import { useWorksheetStore } from '@/features/core/worksheet/worksheetStore';
import {
  buildGraphResourceMeta,
  resourceKey,
  useDocumentStateStore,
  useResourceStore,
  type DocumentState,
  type ProjectResourceMeta,
  type ResourceKey,
} from '@/features/core/resource';
import { useEditorTabStore, type EditorTabMemento } from '@/features/core/layout/editorTabStore';
import {
  remapPlacementActiveTab,
  replacePlacementActiveTab,
} from '@/features/core/layout/editorGraphSelectionPlacement';
import { useGraphSessionStore } from '@/features/core/graphSession/graphSessionStore';
import { useViewportStore } from '@/features/core/viewport';
import { useEditorStore } from '@/features/core/editor/stores/useEditorStore';
import { parseViewportScopeKey, viewportScopeKey } from '@/features/core/viewport/viewportScope';



type UnknownRecord = Record<string, unknown>;

function isRecord(value: unknown): value is UnknownRecord {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}


function sameValue(left: unknown, right: unknown): boolean {
  return JSON.stringify(left) === JSON.stringify(right);
}

function variableDocument(variable: Variable): Omit<Variable, 'resourcePath'> {
  const { resourcePath: _projectionMetadata, ...document } = variable;
  return document;
}

function sameVariableDocument(left: Variable, right: Variable): boolean {
  return sameValue(variableDocument(left), variableDocument(right));
}

function canonicalize(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(canonicalize);
  if (isRecord(value)) {
    return Object.fromEntries(
      Object.entries(value)
        .sort(([left], [right]) => left.localeCompare(right))
        .map(([key, entry]) => [key, canonicalize(entry)]),
    );
  }
  return value;
}

export function collectResourceMutationGraphPaths(
  result: unknown,
  fallbackPaths: Iterable<string> = [],
): Set<string> {
  const paths = new Set(fallbackPaths);
  if (!isRecord(result)) return paths;
  const moves = Array.isArray(result.moves) ? result.moves : [result.moves];
  for (const move of moves) {
    if (!isRecord(move)) continue;
    if (isGraphResourcePath(move.from)) paths.add(move.from);
    if (isGraphResourcePath(move.to)) paths.add(move.to);
  }
  const deltas = Array.isArray(result.deltas) ? result.deltas : [result.deltas];
  for (const delta of deltas) {
    if (!isRecord(delta) || !isRecord(delta.resource)) continue;
    if ((delta.resource.kind === 'graph' || delta.resource.kind === 'function')
      && isGraphResourcePath(delta.resource.key)) paths.add(delta.resource.key);
  }
  const replacements = Array.isArray(result.projectionReplacements)
    ? result.projectionReplacements
    : [result.projectionReplacements];
  for (const replacement of replacements) {
    if (isRecord(replacement) && isGraphResourcePath(replacement.graphPath)) {
      paths.add(replacement.graphPath);
    }
  }
  if (isRecord(result.projectionStatus)) {
    for (const key of ['expectedGraphPaths', 'invalidatedGraphPaths'] as const) {
      const declared = result.projectionStatus[key];
      const entries = Array.isArray(declared) ? declared : [declared];
      for (const path of entries) if (isGraphResourcePath(path)) paths.add(path);
    }
  }
  return paths;
}


export function fingerprintResourceMutationResult(result: ResourceMutationResultDto): string {
  return JSON.stringify(canonicalize(result));
}

function prepareFunctionInstalls(
  deltas: ResourceDeltaDto[],
  replacements: GraphProjectionReplacementDto[],
): PreparedFunctionDeltaInstall[] {
  const graphs = useGraphMetaStore.getState().graphs;
  const functionProjections = new Map(replacements.flatMap((replacement) =>
    'functionEditorProjection' in replacement
      ? [[replacement.graphPath, replacement.functionEditorProjection] as const]
      : []));
  const installs: PreparedFunctionDeltaInstall[] = [];
  for (const delta of deltas) {
    if (delta.resource.kind !== 'function' || delta.payload.kind !== 'function') continue;
    const current = graphs[delta.resource.key];
    if (!current || current.functionRevision == null || !current.functionSignature) {
      throw new Error(`function metadata for '${delta.resource.key}' is incomplete`);
    }
    if (delta.fromRevision !== current.functionRevision
      || delta.toRevision <= delta.fromRevision
      || !sameValue(delta.payload.patch.before, current.functionSignature)) {
      throw new Error(`function delta for '${delta.resource.key}' is inconsistent`);
    }
    const projection = functionProjections.get(delta.resource.key);
    if (!projection) continue;
    installs.push(installFunctionEditorProjection(
      delta.resource.key,
      delta.payload.patch.after,
      projection,
    ));
  }
  return installs;
}

function prepareVariableInstalls(deltas: ResourceDeltaDto[]): PreparedVariableDeltaInstall[] {
  const variableState = useVariableStore.getState();
  const variables = variableState.variables;
  const revisions = variableState.revisions;
  const installs: PreparedVariableDeltaInstall[] = [];
  for (const delta of deltas) {
    if (delta.resource.kind !== 'variable' || delta.payload.kind !== 'variable') continue;
    const id = delta.resource.key.slice('variables/'.length);
    const current = variables[id] ?? null;
    const patch = delta.payload.patch as VariableDocumentPatchDto;
    const before = patch.before == null
      ? null
      : normalizeVariableFromBackend(
        patch.before as Parameters<typeof normalizeVariableFromBackend>[0],
      );
    const normalizedAfter = patch.after == null
      ? null
      : normalizeVariableFromBackend(
        patch.after as Parameters<typeof normalizeVariableFromBackend>[0],
      );
    const after = normalizedAfter == null
      ? null
      : {
          ...variableDocument(normalizedAfter),
          ...(current?.resourcePath ? { resourcePath: current.resourcePath } : {}),
        };
    if ((before === null) !== (current === null)
      || (revisions[id] ?? 0) !== delta.fromRevision
      || (before && current && !sameVariableDocument(before, current))
      || (before && before.id !== id)
      || (after && after.id !== id)) {
      throw new Error(`variable delta for '${delta.resource.key}' is inconsistent`);
    }
    installs.push({
      id,
      before,
      after,
      fromRevision: delta.fromRevision,
      toRevision: delta.toRevision,
    });
  }
  return installs;
}

function databaseDocumentMatches(
  current: DatabaseRecord,
  expected: DatabaseDocumentDto,
): boolean {
  return current.id === expected.id
    && sameValue(current.engine, expected.engine)
    && current.schemaVersion === expected.schemaVersion
    && current.required === expected.required
    && (expected.name === null || current.name === expected.name);
}

function applyDatabaseDeltasToAggregate(
  aggregate: PublicationAggregate,
  deltas: readonly ResourceDeltaDto[],
): void {
  for (const delta of deltas) {
    if (delta.resource.kind !== 'database' || delta.payload.kind !== 'database') continue;
    const { before, after } = delta.payload.patch;
    const id = before?.id ?? after?.id;
    if (!id) throw new Error('database delta omitted its document identity');
    const current = aggregate.databases[id] ?? null;
    if ((before === null) !== (current === null)
      || (aggregate.databaseRevisions[id] ?? 0) !== delta.fromRevision
      || (before && current && !databaseDocumentMatches(current, before))) {
      throw new Error(`database delta for '${delta.resource.key}' is inconsistent`);
    }
    if (after) {
      aggregate.databases[id] = {
        ...normalizeDatabaseRecord(id, after, current ?? undefined),
        resourcePath: delta.resource.key,
      };
      aggregate.databaseRevisions[id] = delta.toRevision;
      const key = resourceKey({ id, kind: 'database' });
      const previous = aggregate.resources[key];
      aggregate.resources[key] = {
        id,
        kind: 'database',
        name: aggregate.databases[id].name,
        uri: key,
        exists: true,
        loaded: true,
        hasDirtyDocument: previous?.hasDirtyDocument ?? false,
        hasStaleDocument: previous?.hasStaleDocument ?? false,
        hasConflictDocument: previous?.hasConflictDocument ?? false,
      };
    } else {
      delete aggregate.databases[id];
      delete aggregate.databaseRevisions[id];
      delete aggregate.resources[resourceKey({ id, kind: 'database' })];
    }
  }
}

function validateMoveCorrelation(result: ResourceMutationResultDto): void {
  const sources = new Set<string>();
  const destinations = new Set<string>();
  for (const move of result.moves) {
    if (sources.has(move.from)) throw new Error(`conflicting move source '${move.from}'`);
    if (destinations.has(move.to)) throw new Error(`conflicting move destination '${move.to}'`);
    sources.add(move.from);
    destinations.add(move.to);
    const correlated = result.deltas.some((delta) =>
      delta.payload.kind === 'resource_move'
      && delta.resource.key === move.to
      && delta.payload.patch.from === move.from
      && delta.payload.patch.to === move.to);
    if (!correlated) throw new Error(`move '${move.from}' to '${move.to}' has no correlated delta`);
  }
}


function removeTabFromMemento(memento: EditorTabMemento, tabId: string): void {
  delete memento.registry[tabId];
  for (const [groupId, placement] of Object.entries(memento.placements)) {
    const closingIndex = placement.tabIds.indexOf(tabId);
    if (closingIndex < 0) continue;
    placement.tabIds = placement.tabIds.filter((id) => id !== tabId);
    placement.selectedTabIds = placement.selectedTabIds.filter((id) => id !== tabId);
    if (placement.activeTabId === tabId) {
      replacePlacementActiveTab(
        placement,
        placement.tabIds[Math.max(0, closingIndex - 1)] ?? null,
      );
    }
    if (placement.tabIds.length === 0) delete memento.placements[groupId];
  }
}

interface PublicationAggregate {
  graphEntities?: Record<string, GraphEntityBucket>;
  resources: Record<ResourceKey, ProjectResourceMeta>;
  graphOrder: string[];
  documents: Record<ResourceKey, DocumentState>;
  graphMeta?: ReturnType<typeof useGraphMetaStore.getState>['graphs'];
  databases: ReturnType<typeof useDatabaseStore.getState>['databases'];
  databaseRevisions: ReturnType<typeof useDatabaseStore.getState>['revisions'];
  variables: ReturnType<typeof useVariableStore.getState>['variables'];
  variableRevisions: ReturnType<typeof useVariableStore.getState>['revisions'];
  worksheetIndex: WorksheetIndexEntry[];
  worksheetDocuments: Record<string, WorksheetDocument>;
  tabs: EditorTabMemento;
  focusedSession?: ReturnType<typeof useGraphSessionStore.getState>['focusedSession'];
  viewports?: ReturnType<typeof useViewportStore.getState>['viewports'];
}

function createPublicationAggregate(
  graphProjectionPlan?: NonNullable<PreparedProjectPublication['graphProjectionPlan']>,
): PublicationAggregate {
  const worksheet = useWorksheetStore.getState();
  const graphOwnedState = graphProjectionPlan
    ? {
        graphEntities: structuredClone(graphProjectionPlan.graphEntities) as Record<string, GraphEntityBucket>,
        graphMeta: structuredClone(useGraphMetaStore.getState().graphs),
        focusedSession: structuredClone(useGraphSessionStore.getState().focusedSession),
        viewports: structuredClone(useViewportStore.getState().viewports),
      }
    : {};
  return {
    ...graphOwnedState,
    resources: structuredClone(useResourceStore.getState().resources) as Record<ResourceKey, ProjectResourceMeta>,
    graphOrder: [...useResourceStore.getState().graphOrder],
    documents: structuredClone(useDocumentStateStore.getState().documents) as Record<ResourceKey, DocumentState>,
    databases: structuredClone(useDatabaseStore.getState().databases),
    databaseRevisions: structuredClone(useDatabaseStore.getState().revisions),
    variables: structuredClone(useVariableStore.getState().variables),
    variableRevisions: structuredClone(useVariableStore.getState().revisions),
    worksheetIndex: structuredClone(worksheet.index),
    worksheetDocuments: structuredClone(worksheet.documents),
    tabs: structuredClone(useEditorTabStore.getState().snapshotMemento()),
  };
}

function renameTabInMemento(tabs: EditorTabMemento, from: string, to: string): void {
  const source = tabs.registry[from];
  if (source && tabs.registry[to]) throw new Error(`move tab destination '${to}' already exists`);
  if (source) {
    tabs.registry[to] = { ...source, id: to };
    delete tabs.registry[from];
  }
  for (const placement of Object.values(tabs.placements)) {
    placement.tabIds = placement.tabIds.map((id) => id === from ? to : id);
    placement.selectedTabIds = placement.selectedTabIds.map((id) => id === from ? to : id);
    remapPlacementActiveTab(placement, from, to);
  }
}

function remapAggregateViewports(
  viewports: NonNullable<PublicationAggregate['viewports']>,
  from: string,
  to: string,
): void {
  for (const key of Object.keys(viewports)) {
    const scope = parseViewportScopeKey(key);
    if (!scope || scope.graphPath !== from) continue;
    const destinationKey = viewportScopeKey({ ...scope, graphPath: to });
    if (viewports[destinationKey]) throw new Error(`move viewport destination '${destinationKey}' exists`);
    viewports[destinationKey] = viewports[key];
    delete viewports[key];
  }
}

function collectRemovedWorksheetPaths(
  deltas: readonly ResourceDeltaDto[],
): ReadonlySet<string> {
  return new Set(deltas.flatMap((delta) => {
    if (delta.resource.kind !== 'worksheet'
      || delta.payload.kind !== 'resource_lifecycle'
      || delta.payload.patch.before?.kind !== 'worksheet'
      || delta.payload.patch.after !== null) return [];
    return [delta.payload.patch.before.path];
  }));
}

function applyResourceLifecycleDeltasToAggregate(
  aggregate: PublicationAggregate,
  deltas: readonly ResourceDeltaDto[],
): void {
  for (const delta of deltas) {
    if (delta.payload.kind !== 'resource_lifecycle') continue;
    const { before, after } = delta.payload.patch;
    const state = before ?? after;
    if (!state) throw new Error(`resource lifecycle delta for '${delta.resource.key}' is empty`);
    if (state.kind === 'worksheet') {
      if (delta.resource.kind !== 'worksheet') {
        throw new Error(`worksheet lifecycle delta for '${state.path}' has mismatched identity`);
      }
      const key = resourceKey({ id: state.path, kind: 'worksheet' });
      if (before === null) {
        if (aggregate.resources[key]) {
          throw new Error(`worksheet lifecycle insert target '${state.path}' already exists`);
        }
        const document = aggregate.worksheetDocuments[state.path];
        if (!document) {
          throw new Error(`worksheet lifecycle insert source '${state.path}' is not loaded`);
        }
        if (aggregate.worksheetIndex.some((entry) => entry.worksheetPath === state.path)) {
          throw new Error(`worksheet lifecycle insert index '${state.path}' already exists`);
        }
        const previousDocumentState = aggregate.documents[key];
        aggregate.documents[key] = {
          ...previousDocumentState,
          resourceKey: key,
          loaded: true,
          dirty: false,
          stale: false,
          missing: false,
          conflict: false,
          version: (previousDocumentState?.version ?? 0) + 1,
          lastLoadedAt: Date.now(),
        };
        aggregate.resources[key] = {
          id: state.path,
          kind: 'worksheet',
          name: state.name,
          uri: key,
          revision: state.revision,
          exists: true,
          loaded: true,
          hasDirtyDocument: false,
          hasStaleDocument: false,
          hasConflictDocument: false,
        };
        aggregate.worksheetIndex.push({
          worksheetPath: state.path,
          name: state.name,
          databaseId: document.databaseId,
          chartType: document.chartType,
          revision: state.revision,
        });

      } else {
        const current = aggregate.resources[key];
        if (!current) {
          throw new Error(`worksheet lifecycle remove source '${state.path}' is inconsistent`);
        }
        delete aggregate.resources[key];
        delete aggregate.documents[key];
        delete aggregate.worksheetDocuments[state.path];
        aggregate.worksheetIndex = aggregate.worksheetIndex.filter(
          (candidate) => candidate.worksheetPath !== state.path,
        );
        removeTabFromMemento(aggregate.tabs, state.path);
      }
      continue;
    }
    if (delta.resource.kind !== 'graph') {
      throw new Error(`graph lifecycle delta for '${state.path}' has mismatched identity`);
    }
    const graphEntities = aggregate.graphEntities;
    const graphMeta = aggregate.graphMeta;
    const viewports = aggregate.viewports;
    if (!graphEntities || !graphMeta || !viewports) {
      throw new Error(`graph lifecycle delta for '${state.path}' has no graph-owned publication state`);
    }
    const key = resourceKey({ id: state.path, kind: state.kind });
    const current = aggregate.resources[key];
    if (before === null) {
      if (current || graphMeta[state.path]
        || aggregate.graphOrder.includes(state.path)) {
        throw new Error(`graph lifecycle insert target '${state.path}' already exists`);
      }
      aggregate.resources[key] = buildGraphResourceMeta(state.kind, state.path, state.name, {
        revision: state.revision,
        loaded: false,
      });
      aggregate.graphOrder.push(state.path);
      graphMeta[state.path] = { path: state.path, name: state.name, type: state.kind };
      continue;
    }
    const projectionRevision = graphEntities[before.path]?.basis.graphRevision;
    const authoritativeRevision = projectionRevision ?? current?.revision;
    if (!current || authoritativeRevision !== before.revision || current.kind !== before.kind) {
      throw new Error(`graph lifecycle remove source '${before.path}' is inconsistent`);
    }
    delete aggregate.resources[key];
    aggregate.graphOrder = aggregate.graphOrder.filter((path) => path !== before.path);
    delete aggregate.documents[key];
    delete graphMeta[before.path];
    delete graphEntities[before.path];
    if (aggregate.focusedSession?.graphPath === before.path) aggregate.focusedSession = null;
    removeTabFromMemento(aggregate.tabs, before.path);
    for (const viewportKey of Object.keys(viewports)) {
      if (parseViewportScopeKey(viewportKey)?.graphPath === before.path) {
        delete viewports[viewportKey];
      }
    }
  }
}

function applyMovesToAggregate(
  aggregate: PublicationAggregate,
  moves: PreparedProjectPublication['moves'],
  deltas: readonly ResourceDeltaDto[],
): void {
  for (const move of moves) {
    if (move.kind === 'worksheet') {
      const fromKey = resourceKey({ id: move.from, kind: 'worksheet' });
      const toKey = resourceKey({ id: move.to, kind: 'worksheet' });
      const source = aggregate.resources[fromKey];
      const destination = move.resources[toKey];
      if (!source || aggregate.resources[toKey] || !destination) {
        throw new Error(`move aggregate resource identity is inconsistent for '${move.from}'`);
      }
      const moveDelta = deltas.find((delta) => delta.resource.kind === 'worksheet'
        && delta.resource.key === move.to
        && delta.payload.kind === 'resource_move'
        && delta.payload.patch.from === move.from
        && delta.payload.patch.to === move.to);
      if (!moveDelta) throw new Error(`worksheet move '${move.from}' has no correlated delta`);
      const destinationDocument = move.documents[move.to];
      if (destinationDocument && destinationDocument.revision !== moveDelta.fromRevision) {
        throw new Error(
          `worksheet move document revision for '${move.from}' does not match fromRevision`,
        );
      }
      if (destination.revision !== moveDelta.fromRevision) {
        throw new Error(
          `worksheet move resource revision for '${move.from}' does not match fromRevision`,
        );
      }
      delete aggregate.resources[fromKey];
      aggregate.resources[toKey] = { ...destination, revision: moveDelta.toRevision };
      delete aggregate.documents[fromKey];
      const destinationState = move.documentStates[toKey];
      if (destinationState) aggregate.documents[toKey] = destinationState;
      delete aggregate.worksheetDocuments[move.from];
      if (destinationDocument) {
        aggregate.worksheetDocuments[move.to] = {
          ...destinationDocument,
          revision: moveDelta.toRevision,
        };
      }
      aggregate.worksheetIndex = aggregate.worksheetIndex.map(
        (entry) => entry.worksheetPath === move.from
        ? {
            ...entry,
            worksheetPath: move.to,
            name: move.name,
            revision: moveDelta.toRevision,
          }
        : entry);
      renameTabInMemento(aggregate.tabs, move.from, move.to);
      continue;
    }
    const graphMeta = aggregate.graphMeta;
    const viewports = aggregate.viewports;
    if (!graphMeta || !viewports) {
      throw new Error(`graph move '${move.from}' has no graph-owned publication state`);
    }
    const source = aggregate.resources[move.resourceSnapshot.fromKey];
    if (!source || aggregate.resources[move.resourceSnapshot.toKey]) {
      throw new Error(`move aggregate resource identity is inconsistent for '${move.from}'`);
    }
    delete aggregate.resources[move.resourceSnapshot.fromKey];
    aggregate.resources[move.resourceSnapshot.toKey] = move.resourceSnapshot.destinationAfter;
    aggregate.graphOrder = aggregate.graphOrder.map((path) => path === move.from ? move.to : path);

    delete aggregate.documents[move.documentSnapshot.fromKey];
    if (move.documentSnapshot.destinationAfter) {
      if (aggregate.documents[move.documentSnapshot.toKey]) {
        throw new Error(`move aggregate document destination '${move.to}' exists`);
      }
      aggregate.documents[move.documentSnapshot.toKey] = move.documentSnapshot.destinationAfter;
    }

    if (graphMeta[move.to]) throw new Error(`move aggregate metadata destination '${move.to}' exists`);
    delete graphMeta[move.from];
    graphMeta[move.to] = move.graphMetaSnapshot.destinationAfter;


    if (aggregate.focusedSession?.graphPath === move.from) {
      aggregate.focusedSession = { ...aggregate.focusedSession, graphPath: move.to };
    }
    renameTabInMemento(aggregate.tabs, move.from, move.to);
    remapAggregateViewports(viewports, move.from, move.to);
  }
}

function markPreparedVariableScopeDirty(
  aggregate: PublicationAggregate,
  scope: Variable['scope'],
): void {
  const graphPath = scope.type === 'event'
    ? scope.eventPath
    : scope.type === 'function' ? scope.functionPath : null;
  if (!graphPath) return;
  const kind = aggregate.graphMeta?.[graphPath]?.type;
  if (kind !== 'event' && kind !== 'function') return;
  const key = resourceKey({ id: graphPath, kind });
  const resource = aggregate.resources[key];
  if (resource) aggregate.resources[key] = { ...resource, hasDirtyDocument: true };
  const document = aggregate.documents[key];
  if (document) aggregate.documents[key] = { ...document, dirty: true };
  const tab = aggregate.tabs.registry[graphPath];
  if (tab) aggregate.tabs.registry[graphPath] = { ...tab, pinned: true };
}

function worksheetMatchesPatchState(
  document: WorksheetDocument,
  state: Extract<ResourceDeltaDto['payload'], { kind: 'worksheet' }>['patch']['before'],
): boolean {
  return document.databaseId === state.databaseId
    && document.chartType === state.chartType
    && sameValue(document.encodings, state.encodings);
}

function applyWorksheetDeltasToAggregate(
  aggregate: PublicationAggregate,
  deltas: readonly ResourceDeltaDto[],
): void {
  for (const delta of deltas) {
    if (delta.resource.kind !== 'worksheet' || delta.payload.kind !== 'worksheet') continue;
    const id = delta.resource.key;
    const existing = aggregate.worksheetDocuments[id];
    const key = resourceKey({ id, kind: 'worksheet' });
    if (!existing || existing.revision !== delta.fromRevision) {
      throw new Error(`worksheet delta for '${id}' is inconsistent`);
    }
    const documentState = aggregate.documents[key];
    const resource = aggregate.resources[key];
    if (!documentState || !resource) {
      throw new Error(`worksheet delta for '${id}' has incomplete projection state`);
    }
    const hasMatchingBefore = worksheetMatchesPatchState(existing, delta.payload.patch.before);
    const hasMatchingAfter = worksheetMatchesPatchState(existing, delta.payload.patch.after);
    if (!hasMatchingBefore && !hasMatchingAfter && documentState?.dirty !== true) {
      throw new Error(`worksheet delta for '${id}' is inconsistent`);
    }
    const matchesAuthoritativeSave = hasMatchingBefore || hasMatchingAfter;
    const after = hasMatchingBefore
      ? {
          ...existing,
          ...structuredClone(delta.payload.patch.after),
          revision: delta.toRevision,
        }
      : { ...existing, revision: delta.toRevision };
    aggregate.worksheetDocuments[id] = after;
    aggregate.worksheetIndex = aggregate.worksheetIndex.map((candidate) =>
      candidate.worksheetPath === id
        ? {
            ...candidate,
            databaseId: after.databaseId,
            chartType: after.chartType,
            revision: delta.toRevision,
          }
        : candidate);
    aggregate.documents[key] = {
      ...documentState,
      dirty: !matchesAuthoritativeSave,
    };
    aggregate.resources[key] = {
      ...resource,
      revision: delta.toRevision,
      hasDirtyDocument: !matchesAuthoritativeSave,
    };
  }
}

function hasGraphOwnedPublicationWork(
  result: ResourceMutationResultDto,
  context: PreparePublicationContext,
): boolean {
  const declaredGraphPaths = result.projectionStatus.status === 'complete'
    ? result.projectionStatus.expectedGraphPaths
    : result.projectionStatus.invalidatedGraphPaths;
  return result.projectionReplacements.length > 0
    || declaredGraphPaths.length > 0
    || context.moves.some((move) => move.kind !== 'worksheet')
    || result.deltas.some((delta) => delta.resource.kind === 'graph'
      || delta.resource.kind === 'function'
      || delta.resource.kind === 'variable');
}

export function prepareSynchronousPublicationCommit(
  result: ResourceMutationResultDto,
  context: PreparePublicationContext,
): PreparedProjectPublication {
  const wireError = validateResourceMutationWireResult(result);
  if (wireError) throw new Error(wireError);
  if (result.projectionStatus.status === 'incomplete') {
    throw new Error('incomplete projection status requires recovery');
  }
  if (result.projectInstanceId !== context.projectInstanceId) {
    throw new Error('publication project identity changed during preparation');
  }
  if (context.moves.length !== result.moves.length) {
    throw new Error('prepared move count does not match publication');
  }
  validateMoveCorrelation(result);
  for (let index = 0; index < context.moves.length; index += 1) {
    const move = result.moves[index];
    const prepared = context.moves[index];
    if (prepared.from !== move.from || prepared.to !== move.to
      || prepared.kind !== move.kind || prepared.name !== move.name) {
      throw new Error('prepared move identity disagrees with publication');
    }
  }
  const graphOwned = hasGraphOwnedPublicationWork(result, context);
  let graphProjectionPlan: NonNullable<PreparedProjectPublication['graphProjectionPlan']> | undefined;
  if (graphOwned) {
    const baseGraphEntities = useGraphDataStore.getState().graphEntities;
    const projectedRevisions = new Map(
      Object.entries(baseGraphEntities)
        .map(([path, bucket]) => [path, bucket.sourceRevision] as const),
    );
    for (const replacement of result.projectionReplacements) {
      const entities = toProjectionEntities(replacement.projection);
      if (entities.graphPath !== replacement.graphPath) {
        throw new Error(`replacement for '${replacement.graphPath}' has invalid projection identity`);
      }
      const currentRevision = projectedRevisions.get(replacement.graphPath);
      if (currentRevision != null && replacement.projection.sourceRevision < currentRevision) {
        throw new Error(`replacement for '${replacement.graphPath}' is older than prepared authority`);
      }
      projectedRevisions.set(replacement.graphPath, replacement.projection.sourceRevision);
    }
    const preparedGraph = prepareGraphProjectionReplacements(
      result.projectionReplacements,
      baseGraphEntities,
    );
    if (!preparedGraph.prepared) {
      throw new Error(`projection preparation failed for '${preparedGraph.graphPath}'`);
    }
    const graphEntities = { ...preparedGraph.plan.graphEntities };
    for (const move of context.moves) {
      if (move.kind !== 'worksheet') delete graphEntities[move.from];
    }
    graphProjectionPlan = { ...preparedGraph.plan, graphEntities };
  }
  const aggregate = createPublicationAggregate(graphProjectionPlan);
  applyMovesToAggregate(aggregate, context.moves, result.deltas);
  for (const replacement of result.projectionReplacements) {
    const kind = inferGraphResourceKind(replacement.graphPath);
    if (!kind) continue;
    const key = resourceKey({ id: replacement.graphPath, kind });
    const resource = aggregate.resources[key];
    if (resource) {
      aggregate.resources[key] = {
        ...resource,
        revision: replacement.projection.basis.graphRevision,
      };
    }
  }
  applyResourceLifecycleDeltasToAggregate(aggregate, result.deltas);

  const functionInstalls = graphOwned
    ? prepareFunctionInstalls(result.deltas, result.projectionReplacements)
    : [];
  for (const install of functionInstalls) {
    const graphMeta = aggregate.graphMeta;
    const graph = graphMeta?.[install.graphPath];
    if (!graph || !graphMeta) throw new Error(`function metadata target '${install.graphPath}' is absent`);
    graphMeta[install.graphPath] = {
      ...graph,
      functionRevision: install.revision,
      functionSignature: install.signature,
      functionInputs: [...install.functionInputs],
      functionOutputs: [...install.functionOutputs],
    };
  }
  const variableInstalls = prepareVariableInstalls(result.deltas);
  for (const install of variableInstalls) {
    if (install.before) markPreparedVariableScopeDirty(aggregate, install.before.scope);
    aggregate.variableRevisions[install.id] = install.toRevision;
    if (install.after) {
      aggregate.variables[install.id] = install.after;
      markPreparedVariableScopeDirty(aggregate, install.after.scope);
      const [meta] = variableCatalogToResourceMetas({ [install.id]: install.after });
      aggregate.resources[meta.uri] = meta;
    } else {
      delete aggregate.variables[install.id];
      delete aggregate.resources[`yssbi://variable/${install.id}`];
    }
  }
  applyDatabaseDeltasToAggregate(aggregate, result.deltas);
  applyWorksheetDeltasToAggregate(aggregate, result.deltas);

  return {
    projectInstanceId: context.projectInstanceId,
    epoch: context.epoch,
    publicationRevision: result.publicationRevision,
    fingerprint: context.fingerprint,
    affectedGraphPaths: context.affectedGraphPaths,
    moves: context.moves,
    removedWorksheetPaths: collectRemovedWorksheetPaths(result.deltas),
    ...(graphProjectionPlan && aggregate.graphEntities
      ? { graphProjectionPlan: { ...graphProjectionPlan, graphEntities: aggregate.graphEntities } }
      : {}),
    projectionReplacements: result.projectionReplacements,
    functionInstalls,
    variableInstalls,
    storeState: {
      resources: aggregate.resources,
      graphOrder: aggregate.graphOrder,
      documents: aggregate.documents,
      ...(aggregate.graphMeta ? { graphMeta: aggregate.graphMeta } : {}),
      databases: aggregate.databases,
      databaseRevisions: aggregate.databaseRevisions,
      variables: aggregate.variables,
      variableRevisions: aggregate.variableRevisions,
      worksheetIndex: aggregate.worksheetIndex,
      worksheetDocuments: aggregate.worksheetDocuments,
      tabs: aggregate.tabs,
      ...('focusedSession' in aggregate ? { focusedSession: aggregate.focusedSession } : {}),
      ...(aggregate.viewports ? { viewports: aggregate.viewports } : {}),
    },
    history: result.history,
  };
}

export function commitPreparedPublication(plan: PreparedProjectPublication): void {
  if (plan.graphProjectionPlan) {
    commitPreparedGraphProjectionReplacements(plan.graphProjectionPlan);
  }
  useResourceStore.setState({
    resources: plan.storeState.resources,
    graphOrder: plan.storeState.graphOrder,
  });
  useDocumentStateStore.setState({ documents: plan.storeState.documents });
  if (plan.storeState.graphMeta) {
    useGraphMetaStore.setState({ graphs: plan.storeState.graphMeta });
  }
  useDatabaseStore.setState({
    databases: plan.storeState.databases,
    revisions: plan.storeState.databaseRevisions,
  });
  useVariableStore.setState({
    variables: plan.storeState.variables,
    revisions: plan.storeState.variableRevisions,
  });
  useWorksheetStore.setState({
    index: plan.storeState.worksheetIndex,
    documents: plan.storeState.worksheetDocuments,
  });
  if ('focusedSession' in plan.storeState) {
    useGraphSessionStore.setState({ focusedSession: plan.storeState.focusedSession ?? null });
  }
  useEditorTabStore.setState(plan.storeState.tabs);
  if (plan.storeState.viewports) {
    useViewportStore.setState({ viewports: plan.storeState.viewports });
  }
  const detailFocus = useEditorStore.getState().detailFocus;
  if (detailFocus?.kind === 'worksheet'
    && plan.removedWorksheetPaths.has(detailFocus.worksheetPath)) {
    useEditorStore.getState().clearDetailFocus();
  }
  for (const move of plan.moves) {
    if (move.kind === 'worksheet') {
      remapWorksheetNonViewportUiState(move.from, move.to);
      invalidateWorksheetPreviewCacheForMove(plan.projectInstanceId, move.from, move.to);
    } else {
      remapGraphNonViewportUiState(move.from, move.to);
    }
  }
}
