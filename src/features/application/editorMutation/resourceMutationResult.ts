import {
  commitPreparedGraphProjectionReplacements,
  prepareGraphProjectionReplacements,
  useGraphDataStore,
  type GraphEntityBucket,
} from '@/features/core/dataStore/graphDataStore';
import { useGraphMetaStore } from '@/features/core/dataStore/graphMetaStore';
import { useVariableStore } from '@/features/core/dataStore/variableStore';

import { areResourceDeltasValid } from '@/features/core/sync/utils/resourceMutationWireValidator';
import { inferGraphResourceKind } from '@/shared/types/domain/graphResourcePath';
import { toProjectionEntities } from '@/features/domain/editorProjection';
import { normalizeVariableFromBackend } from '@/shared/types/dto/variable';
import { variableCatalogToResourceMetas } from '@/features/core/variable/variableCatalog';
import type {
  GraphProjectionReplacementDto,
  ResourceDeltaDto,
  ResourceMutationResultDto,
  VariableDocumentPatchDto,
  WorksheetDeltaDto,
} from '@/shared/types/dto/editorMutation';
import type { Variable } from '@/shared/types/domain/variable';
import type { WorksheetDocument, WorksheetIndexEntry } from '@/shared/types/domain/worksheet';
import {
  functionSignaturePins,
} from '@/features/application/graphDocument/functionSignatureSync';

import type {
  PreparedFunctionDeltaInstall,
  PreparedProjectPublication,
  PreparePublicationContext,
  PreparedVariableDeltaInstall,
  PreparedWorksheetDeltaInstall,
  PreparedWorksheetPublicationState,
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
import { useGraphSessionStore } from '@/features/core/graphSession/graphSessionStore';
import { useViewportStore } from '@/features/core/viewport';
import { parseViewportScopeKey, viewportScopeKey } from '@/features/core/viewport/viewportScope';



type UnknownRecord = Record<string, unknown>;

function isRecord(value: unknown): value is UnknownRecord {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function isGraphPath(value: unknown): value is string {
  return typeof value === 'string' && inferGraphResourceKind(value) != null;
}

function isWorksheetDocument(value: unknown): value is WorksheetDocument {
  return isRecord(value)
    && Number.isSafeInteger(value.schemaVersion)
    && Number.isSafeInteger(value.revision)
    && typeof value.id === 'string'
    && value.id.length > 0
    && typeof value.name === 'string'
    && typeof value.databaseId === 'string'
    && (value.chartType === 'histogram' || value.chartType === 'scatter' || value.chartType === 'line')
    && isRecord(value.encodings)
    && (value.encodings.x === undefined || typeof value.encodings.x === 'string')
    && (value.encodings.y === undefined || typeof value.encodings.y === 'string');
}

function validateWorksheetDeltas(value: unknown): string | undefined {
  if (value === undefined) return undefined;
  if (!Array.isArray(value)) return 'worksheet deltas are malformed';
  const ids = new Set<string>();
  for (const delta of value) {
    if (!isRecord(delta)
      || typeof delta.id !== 'string'
      || !delta.id
      || (delta.before !== null && !isWorksheetDocument(delta.before))
      || (delta.after !== null && !isWorksheetDocument(delta.after))
      || (delta.before === null && delta.after === null)
      || (isWorksheetDocument(delta.before) && delta.before.id !== delta.id)
      || (isWorksheetDocument(delta.after) && delta.after.id !== delta.id)
      || ids.has(delta.id)) {
      return 'worksheet deltas are malformed';
    }
    ids.add(delta.id);
  }
  return undefined;
}

function sameValue(left: unknown, right: unknown): boolean {
  return JSON.stringify(left) === JSON.stringify(right);
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
    if (isGraphPath(move.from)) paths.add(move.from);
    if (isGraphPath(move.to)) paths.add(move.to);
  }
  const deltas = Array.isArray(result.deltas) ? result.deltas : [result.deltas];
  for (const delta of deltas) {
    if (!isRecord(delta) || !isRecord(delta.resource)) continue;
    if ((delta.resource.kind === 'graph' || delta.resource.kind === 'function')
      && isGraphPath(delta.resource.key)) paths.add(delta.resource.key);
  }
  const replacements = Array.isArray(result.projectionReplacements)
    ? result.projectionReplacements
    : [result.projectionReplacements];
  for (const replacement of replacements) {
    if (isRecord(replacement) && isGraphPath(replacement.graphPath)) {
      paths.add(replacement.graphPath);
    }
  }
  if (isRecord(result.projectionStatus)) {
    for (const key of ['expectedGraphPaths', 'invalidatedGraphPaths'] as const) {
      const declared = result.projectionStatus[key];
      const entries = Array.isArray(declared) ? declared : [declared];
      for (const path of entries) if (isGraphPath(path)) paths.add(path);
    }
  }
  return paths;
}

function validateUniqueGraphPaths(value: unknown, label: string): string[] | string {
  if (!Array.isArray(value) || !value.every(isGraphPath)) return `${label} are malformed`;
  if (new Set(value).size !== value.length) return `${label} contain duplicates`;
  return value;
}

function graphPathFromDelta(delta: ResourceDeltaDto): string | undefined {
  return delta.resource.kind === 'graph' || delta.resource.kind === 'function'
    ? delta.resource.key
    : undefined;
}

function validateReplacement(
  replacement: GraphProjectionReplacementDto,
  deltas: ResourceDeltaDto[],
): string | undefined {
  if (!isRecord(replacement)
    || !isGraphPath(replacement.graphPath)
    || !isRecord(replacement.projection)
    || replacement.projection.graphPath !== replacement.graphPath
    || replacement.projection.basis?.graphPath !== replacement.graphPath
    || !Number.isSafeInteger(replacement.projection.sourceRevision)
    || replacement.projection.sourceRevision < 0) {
    return 'projection replacement path identity is malformed';
  }
  const delta = deltas.find((candidate) => graphPathFromDelta(candidate) === replacement.graphPath);
  if (delta?.resource.kind === 'graph'
    && replacement.projection.sourceRevision !== delta.toRevision) {
    return `replacement for '${replacement.graphPath}' disagrees with its graph delta`;
  }
  return undefined;
}

export function validateResourceMutationWireResult(
  result: ResourceMutationResultDto,
): string | undefined {
  if (!isRecord(result)) return 'resource mutation result is malformed';
  if (typeof result.operationId !== 'string' || !result.operationId) {
    return 'operation correlation is malformed';
  }
  if (typeof result.projectInstanceId !== 'string' || !result.projectInstanceId) {
    return 'project instance identity is malformed';
  }
  if (!Number.isSafeInteger(result.publicationRevision) || result.publicationRevision < 1) {
    return 'publication revision is malformed';
  }
  if (!Array.isArray(result.moves) || !result.moves.every((move) =>
    isRecord(move)
    && isGraphPath(move.from)
    && isGraphPath(move.to)
    && move.from !== move.to
    && (move.kind === 'event' || move.kind === 'function')
    && typeof move.name === 'string'
    && move.name.trim().length > 0)) return 'resource moves are malformed';
  if (!areResourceDeltasValid(result.deltas)) return 'resource deltas are malformed';
  if (result.deltas.some((delta) => delta.causedBy !== null && delta.causedBy !== result.operationId)) {
    return 'resource delta operation correlation is inconsistent';
  }
  const worksheetError = validateWorksheetDeltas(result.worksheetDeltas);
  if (worksheetError) return worksheetError;
  if (!Array.isArray(result.projectionReplacements)) return 'projection replacements are malformed';
  if (typeof result.history?.canUndo !== 'boolean' || typeof result.history?.canRedo !== 'boolean') {
    return 'history status is malformed';
  }

  let expectedPaths: string[] | undefined;
  if (result.projectionStatus?.status === 'complete') {
    const validated = validateUniqueGraphPaths(
      result.projectionStatus.expectedGraphPaths,
      'expected graph paths',
    );
    if (typeof validated === 'string') return validated;
    expectedPaths = validated;
  } else if (result.projectionStatus?.status === 'incomplete') {
    const validated = validateUniqueGraphPaths(
      result.projectionStatus.invalidatedGraphPaths,
      'invalidated graph paths',
    );
    if (typeof validated === 'string') return validated;
  } else {
    return 'projection status is malformed';
  }

  const replacementPaths = new Set<string>();
  for (const replacement of result.projectionReplacements) {
    const error = validateReplacement(replacement, result.deltas);
    if (error) return error;
    if (replacementPaths.has(replacement.graphPath)) {
      return `duplicate replacement for '${replacement.graphPath}'`;
    }
    replacementPaths.add(replacement.graphPath);
  }

  if (expectedPaths) {
    const expected = new Set(expectedPaths);
    if (expected.size !== replacementPaths.size
      || [...expected].some((path) => !replacementPaths.has(path))) {
      return 'complete replacement paths do not equal the declared expected graph paths';
    }
    for (const delta of result.deltas) {
      const path = graphPathFromDelta(delta);
      if (path && !expected.has(path)) {
        return `delta path '${path}' is absent from the declared expected graph paths`;
      }
    }
  }
  return undefined;
}

export function fingerprintResourceMutationResult(result: ResourceMutationResultDto): string {
  return JSON.stringify(canonicalize(result));
}

function prepareFunctionInstalls(deltas: ResourceDeltaDto[]): PreparedFunctionDeltaInstall[] {
  const graphs = useGraphMetaStore.getState().graphs;
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
    const pins = functionSignaturePins(delta.payload.patch.after);
    installs.push({
      graphPath: delta.resource.key,
      revision: delta.toRevision,
      signature: delta.payload.patch.after,
      functionInputs: pins.functionInputs,
      functionOutputs: pins.functionOutputs,
    });
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
    const after = patch.after == null
      ? null
      : normalizeVariableFromBackend(
        patch.after as Parameters<typeof normalizeVariableFromBackend>[0],
      );
    if ((before === null) !== (current === null)
      || (revisions[id] ?? 0) !== delta.fromRevision
      || (before && current && !sameValue(before, current))
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

function validateMoveCorrelation(result: ResourceMutationResultDto): void {
  const sources = new Set<string>();
  const destinations = new Set<string>();
  for (const move of result.moves) {
    if (sources.has(move.from)) throw new Error(`conflicting move source '${move.from}'`);
    if (destinations.has(move.to)) throw new Error(`conflicting move destination '${move.to}'`);
    sources.add(move.from);
    destinations.add(move.to);
    const correlated = result.deltas.some((delta) =>
      delta.payload.kind === 'graph_resource_move'
      && delta.resource.key === move.to
      && delta.payload.patch.from === move.from
      && delta.payload.patch.to === move.to);
    if (!correlated) throw new Error(`move '${move.from}' to '${move.to}' has no correlated delta`);
  }
}

function worksheetIndexEntry(document: WorksheetDocument): WorksheetIndexEntry {
  return {
    id: document.id,
    name: document.name,
    databaseId: document.databaseId,
    chartType: document.chartType,
  };
}

function removeTabFromMemento(memento: EditorTabMemento, tabId: string): void {
  delete memento.registry[tabId];
  for (const [groupId, placement] of Object.entries(memento.placements)) {
    const closingIndex = placement.tabIds.indexOf(tabId);
    if (closingIndex < 0) continue;
    placement.tabIds = placement.tabIds.filter((id) => id !== tabId);
    placement.selectedTabIds = placement.selectedTabIds.filter((id) => id !== tabId);
    if (placement.activeTabId === tabId) {
      placement.activeTabId = placement.tabIds[Math.max(0, closingIndex - 1)] ?? null;
    }
    if (placement.tabIds.length === 0) delete memento.placements[groupId];
  }
}

function createPublicationAggregate(graphEntities: PreparedProjectPublication['graphProjectionPlan']['graphEntities']) {
  const worksheet = useWorksheetStore.getState();
  return {
    graphEntities: structuredClone(graphEntities) as Record<string, GraphEntityBucket>,
    resources: structuredClone(useResourceStore.getState().resources) as Record<ResourceKey, ProjectResourceMeta>,
    graphOrder: [...useResourceStore.getState().graphOrder],
    documents: structuredClone(useDocumentStateStore.getState().documents) as Record<ResourceKey, DocumentState>,
    graphMeta: structuredClone(useGraphMetaStore.getState().graphs),
    variables: structuredClone(useVariableStore.getState().variables),
    variableRevisions: structuredClone(useVariableStore.getState().revisions),
    worksheetIndex: structuredClone(worksheet.index),
    worksheetDocuments: structuredClone(worksheet.documents),
    tabs: structuredClone(useEditorTabStore.getState().snapshotMemento()),
    focusedSession: structuredClone(useGraphSessionStore.getState().focusedSession),
    viewports: structuredClone(useViewportStore.getState().viewports),
  };
}

type PublicationAggregate = ReturnType<typeof createPublicationAggregate>;

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
    if (placement.activeTabId === from) placement.activeTabId = to;
  }
}

function remapAggregateViewports(
  viewports: PublicationAggregate['viewports'],
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

function graphLifecycleName(path: string, kind: 'event' | 'function'): string {
  const filename = path.slice(path.lastIndexOf('/') + 1);
  const suffix = kind === 'event' ? '.yssbi-event' : '.yssbi-function';
  return filename.endsWith(suffix) ? filename.slice(0, -suffix.length) : filename;
}

function applyGraphLifecycleDeltasToAggregate(
  aggregate: PublicationAggregate,
  deltas: readonly ResourceDeltaDto[],
): void {
  for (const delta of deltas) {
    if (delta.resource.kind !== 'graph'
      || delta.payload.kind !== 'graph_resource_lifecycle') continue;
    const { before, after } = delta.payload.patch;
    const state = before ?? after;
    if (!state) throw new Error(`graph lifecycle delta for '${delta.resource.key}' is empty`);
    const key = resourceKey({ id: state.path, kind: state.kind });
    const current = aggregate.resources[key];
    if (before === null) {
      if (current || aggregate.graphMeta[state.path]
        || aggregate.graphOrder.includes(state.path)) {
        throw new Error(`graph lifecycle insert target '${state.path}' already exists`);
      }
      const name = graphLifecycleName(state.path, state.kind);
      aggregate.resources[key] = buildGraphResourceMeta(state.kind, state.path, name, {
        revision: state.revision,
        loaded: false,
      });
      aggregate.graphOrder.push(state.path);
      aggregate.graphMeta[state.path] = { path: state.path, name, type: state.kind };
      continue;
    }
    if (!current || current.revision !== before.revision || current.kind !== before.kind) {
      throw new Error(`graph lifecycle remove source '${before.path}' is inconsistent`);
    }
    delete aggregate.resources[key];
    aggregate.graphOrder = aggregate.graphOrder.filter((path) => path !== before.path);
    delete aggregate.documents[key];
    delete aggregate.graphMeta[before.path];
    delete aggregate.graphEntities[before.path];
    if (aggregate.focusedSession?.graphPath === before.path) aggregate.focusedSession = null;
    removeTabFromMemento(aggregate.tabs, before.path);
    for (const viewportKey of Object.keys(aggregate.viewports)) {
      if (parseViewportScopeKey(viewportKey)?.graphPath === before.path) {
        delete aggregate.viewports[viewportKey];
      }
    }
  }
}

function applyMovesToAggregate(
  aggregate: PublicationAggregate,
  moves: PreparedProjectPublication['moves'],
): void {
  for (const move of moves) {
    const source = aggregate.resources[move.resourceSnapshot.fromKey];
    if (!source || aggregate.resources[move.resourceSnapshot.toKey]) {
      throw new Error(`move aggregate resource identity is inconsistent for '${move.from}'`);
    }
    delete aggregate.resources[move.resourceSnapshot.fromKey];
    aggregate.resources[move.resourceSnapshot.toKey] = move.resourceSnapshot.destinationAfterLoadedMark;
    aggregate.graphOrder = aggregate.graphOrder.map((path) => path === move.from ? move.to : path);

    delete aggregate.documents[move.documentSnapshot.fromKey];
    if (move.documentSnapshot.destinationAfterLoadedMark) {
      if (aggregate.documents[move.documentSnapshot.toKey]) {
        throw new Error(`move aggregate document destination '${move.to}' exists`);
      }
      aggregate.documents[move.documentSnapshot.toKey] = move.documentSnapshot.destinationAfterLoadedMark;
    }

    if (aggregate.graphMeta[move.to]) throw new Error(`move aggregate metadata destination '${move.to}' exists`);
    delete aggregate.graphMeta[move.from];
    aggregate.graphMeta[move.to] = move.graphMetaSnapshot.destinationAfter;

    for (const install of move.referenceSnapshot.callers) {
      const bucket = aggregate.graphEntities[install.graphPath];
      const node = bucket?.nodes[install.nodeId];
      if (!bucket || !node || node.subGraphPath !== install.before) {
        throw new Error(`move caller reference '${install.graphPath}/${install.nodeId}' changed`);
      }
      aggregate.graphEntities[install.graphPath] = {
        ...bucket,
        nodes: { ...bucket.nodes, [install.nodeId]: { ...node, subGraphPath: install.after } },
      };
    }
    for (const install of move.variableScopeSnapshot.installs) {
      const variable = aggregate.variables[install.id];
      if (!variable || !sameValue(variable.scope, install.before)) {
        throw new Error(`move variable scope '${install.id}' changed`);
      }
      aggregate.variables[install.id] = { ...variable, scope: install.after };
    }
    if (aggregate.focusedSession?.graphPath === move.from) {
      aggregate.focusedSession = { ...aggregate.focusedSession, graphPath: move.to };
    }
    renameTabInMemento(aggregate.tabs, move.from, move.to);
    remapAggregateViewports(aggregate.viewports, move.from, move.to);
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
  const kind = aggregate.graphMeta[graphPath]?.type;
  if (kind !== 'event' && kind !== 'function') return;
  const key = resourceKey({ id: graphPath, kind });
  const resource = aggregate.resources[key];
  if (resource) aggregate.resources[key] = { ...resource, hasDirtyDocument: true };
  const document = aggregate.documents[key];
  if (document) aggregate.documents[key] = { ...document, dirty: true };
  const tab = aggregate.tabs.registry[graphPath];
  if (tab) aggregate.tabs.registry[graphPath] = { ...tab, pinned: true };
}

function applyWorksheetDeltasToAggregate(
  aggregate: PublicationAggregate,
  deltas: readonly WorksheetDeltaDto[],
): PreparedWorksheetDeltaInstall[] {
  const installs: PreparedWorksheetDeltaInstall[] = [];
  for (const delta of deltas) {
    const { id } = delta;
    const existing = aggregate.worksheetDocuments[id] ?? null;
    const key = resourceKey({ id, kind: 'worksheet' });
    if (!sameValue(existing, delta.before)) {
      const documentState = aggregate.documents[key];
      if (existing === null
        || delta.before === null
        || delta.after === null
        || documentState?.dirty !== true
        || existing.revision !== delta.before.revision) {
        throw new Error(`worksheet delta for '${id}' is inconsistent`);
      }
      const preserved = { ...existing, revision: delta.after.revision };
      aggregate.worksheetDocuments[id] = preserved;
      const entry = worksheetIndexEntry(preserved);
      aggregate.worksheetIndex = aggregate.worksheetIndex.some((candidate) => candidate.id === id)
        ? aggregate.worksheetIndex.map((candidate) => candidate.id === id ? entry : candidate)
        : [...aggregate.worksheetIndex, entry];
      const resource = aggregate.resources[key];
      if (resource) {
        aggregate.resources[key] = { ...resource, name: preserved.name, hasDirtyDocument: true };
      }
      installs.push({ id, before: delta.before, after: delta.after });
      continue;
    }
    installs.push({ id, before: delta.before, after: delta.after });
    if (delta.after) {
      aggregate.worksheetDocuments[id] = structuredClone(delta.after);
      const entry = worksheetIndexEntry(delta.after);
      aggregate.worksheetIndex = aggregate.worksheetIndex.some((candidate) => candidate.id === id)
        ? aggregate.worksheetIndex.map((candidate) => candidate.id === id ? entry : candidate)
        : [...aggregate.worksheetIndex, entry];
      aggregate.resources[key] = {
        id,
        kind: 'worksheet',
        name: delta.after.name,
        uri: `yssbi://worksheet/${id}`,
        exists: true,
        loaded: true,
        hasDirtyDocument: false,
        hasStaleDocument: false,
        hasConflictDocument: false,
      };
      aggregate.documents[key] = {
        resourceKey: key,
        loaded: true,
        dirty: false,
        stale: false,
        missing: false,
        conflict: false,
        version: aggregate.documents[key]?.version ?? 0,
      };
    } else {
      delete aggregate.worksheetDocuments[id];
      aggregate.worksheetIndex = aggregate.worksheetIndex.filter((candidate) => candidate.id !== id);
      delete aggregate.resources[key];
      delete aggregate.documents[key];
      removeTabFromMemento(aggregate.tabs, id);
    }
  }
  return installs;
}

export function prepareSynchronousPublicationCommit(
  result: ResourceMutationResultDto,
  context: PreparePublicationContext,
): PreparedProjectPublication {
  const wireError = validateResourceMutationWireResult(result);
  if (wireError) throw new Error(wireError);
  if (result.projectInstanceId !== context.projectInstanceId) {
    throw new Error('publication project identity changed during preparation');
  }
  if (context.moves.length !== result.moves.length) {
    throw new Error('prepared move count does not match publication');
  }
  validateMoveCorrelation(result);
  const baseGraphEntities = useGraphDataStore.getState().graphEntities;
  const projectedRevisions = new Map(
    Object.entries(baseGraphEntities)
      .map(([path, bucket]) => [path, bucket.sourceRevision] as const),
  );
  for (const move of context.moves) {
    projectedRevisions.set(move.to, move.destinationProjection.sourceRevision);
  }
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
  for (let index = 0; index < context.moves.length; index += 1) {
    const move = result.moves[index];
    const prepared = context.moves[index];
    if (prepared.from !== move.from || prepared.to !== move.to
      || prepared.kind !== move.kind || prepared.name !== move.name) {
      throw new Error('prepared move identity disagrees with publication');
    }
  }
  const projectionByPath = new Map<string, GraphProjectionReplacementDto>();
  for (const move of context.moves) {
    projectionByPath.set(move.to, { graphPath: move.to, projection: move.destinationProjection });
  }
  for (const replacement of result.projectionReplacements) {
    projectionByPath.set(replacement.graphPath, replacement);
  }
  const preparedGraph = prepareGraphProjectionReplacements(
    [...projectionByPath.values()],
    baseGraphEntities,
  );
  if (!preparedGraph.prepared) {
    throw new Error(`projection preparation failed for '${preparedGraph.graphPath}'`);
  }
  const graphEntities = { ...preparedGraph.plan.graphEntities };
  for (const move of context.moves) delete graphEntities[move.from];
  const aggregate = createPublicationAggregate(graphEntities);
  applyMovesToAggregate(aggregate, context.moves);
  applyGraphLifecycleDeltasToAggregate(aggregate, result.deltas);

  const functionInstalls = prepareFunctionInstalls(result.deltas);
  for (const install of functionInstalls) {
    const graph = aggregate.graphMeta[install.graphPath];
    if (!graph) throw new Error(`function metadata target '${install.graphPath}' is absent`);
    aggregate.graphMeta[install.graphPath] = {
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
  const worksheetInstalls = applyWorksheetDeltasToAggregate(
    aggregate,
    result.worksheetDeltas ?? [],
  );
  const worksheetState: PreparedWorksheetPublicationState = {
    index: aggregate.worksheetIndex,
    documents: aggregate.worksheetDocuments,
    resources: aggregate.resources,
    documentStates: aggregate.documents,
    tabs: aggregate.tabs,
  };

  return {
    projectInstanceId: context.projectInstanceId,
    epoch: context.epoch,
    publicationRevision: result.publicationRevision,
    fingerprint: context.fingerprint,
    affectedGraphPaths: context.affectedGraphPaths,
    moves: context.moves,
    graphProjectionPlan: {
      graphPaths: preparedGraph.plan.graphPaths,
      graphEntities: aggregate.graphEntities,
    },
    projectionReplacements: result.projectionReplacements,
    functionInstalls,
    variableInstalls,
    worksheetInstalls,
    worksheetState,
    storeState: {
      resources: aggregate.resources,
      graphOrder: aggregate.graphOrder,
      documents: aggregate.documents,
      graphMeta: aggregate.graphMeta,
      variables: aggregate.variables,
      variableRevisions: aggregate.variableRevisions,
      worksheetIndex: aggregate.worksheetIndex,
      worksheetDocuments: aggregate.worksheetDocuments,
      tabs: aggregate.tabs,
      focusedSession: aggregate.focusedSession,
      viewports: aggregate.viewports,
    },
    history: result.history,
  };
}

export function commitPreparedPublication(plan: PreparedProjectPublication): void {
  commitPreparedGraphProjectionReplacements(plan.graphProjectionPlan);
  useResourceStore.setState({
    resources: plan.storeState.resources,
    graphOrder: plan.storeState.graphOrder,
  });
  useDocumentStateStore.setState({ documents: plan.storeState.documents });
  useGraphMetaStore.setState({ graphs: plan.storeState.graphMeta });
  useVariableStore.setState({
    variables: plan.storeState.variables,
    revisions: plan.storeState.variableRevisions,
  });
  useWorksheetStore.setState({
    index: plan.storeState.worksheetIndex,
    documents: plan.storeState.worksheetDocuments,
  });
  useGraphSessionStore.setState({ focusedSession: plan.storeState.focusedSession });
  useEditorTabStore.setState(plan.storeState.tabs);
  useViewportStore.setState({ viewports: plan.storeState.viewports });
}
