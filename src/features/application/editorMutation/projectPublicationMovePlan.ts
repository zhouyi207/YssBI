import type { EditorGraphProjectionDto } from '@/shared/types/dto/editorProjection';
import type { ResourceMoveDto } from '@/shared/types/dto/editorMutation';
import type { Variable } from '@/shared/types';
import { isCallFunctionNodeType } from '@/features/domain/nodeCatalog';
import { toProjectionEntities } from '@/features/domain/editorProjection';
import { normalizeGraphResourcePath } from '@/shared/types/domain/graphResourcePath';
import { useGraphDataStore, useGraphMetaStore, useVariableStore } from '@/features/core/dataStore';
import {
  commitPreparedGraphProjectionReplacements,
  prepareGraphProjectionReplacements,
  type PreparedGraphProjectionReplacements,
} from '@/features/core/dataStore/graphDataStore';
import type { GraphMeta } from '@/features/core/dataStore/graphMetaStore';
import { useGraphSessionStore, type FocusedGraphSession } from '@/features/core/graphSession/graphSessionStore';
import {
  useEditorTabStore,
  type EditorTabMemento,
} from '@/features/core/layout/editorTabStore';
import {
  buildGraphResourceMeta,
  lookupGraphResource,
  resourceKey,
  useDocumentStateStore,
  useResourceStore,
  type DocumentState,
  type ProjectResourceMeta,
  type ResourceKey,
} from '@/features/core/resource';
import { parseViewportScopeKey, viewportScopeKey } from '@/features/core/viewport/viewportScope';
import { useViewportStore } from '@/features/core/viewport/useViewportStore';
import type { EditorViewport } from '@/features/core/viewport/editorViewport';

export interface PreparedResourceMoveSnapshot {
  readonly fromKey: ResourceKey;
  readonly toKey: ResourceKey;
  readonly source: ProjectResourceMeta;
  readonly destinationBefore: undefined;
  readonly destinationBeforeLoadedMark: ProjectResourceMeta;
  readonly destinationAfterLoadedMark: ProjectResourceMeta;
  readonly graphOrderAfter: readonly string[];
}

export interface PreparedDocumentMoveSnapshot {
  readonly fromKey: ResourceKey;
  readonly toKey: ResourceKey;
  readonly source?: DocumentState;
  readonly destinationBefore: undefined;
  readonly destinationBeforeLoadedMark?: DocumentState;
  readonly destinationAfterLoadedMark?: DocumentState;
}

export interface PreparedTabMoveSnapshot {
  readonly before: EditorTabMemento;
  readonly after: EditorTabMemento;
}

export interface PreparedSessionMoveSnapshot {
  readonly before: FocusedGraphSession | null;
  readonly after: FocusedGraphSession | null;
}

interface PreparedCallerReference {
  readonly graphPath: string;
  readonly nodeId: string;
  readonly before: string;
  readonly after: string;
}

export interface PreparedGraphReferenceMoveSnapshot {
  readonly callers: readonly PreparedCallerReference[];
}

interface PreparedVariableScopeInstall {
  readonly id: string;
  readonly before: Variable['scope'];
  readonly after: Variable['scope'];
}

export interface PreparedVariableScopeMoveSnapshot {
  readonly installs: readonly PreparedVariableScopeInstall[];
}

interface PreparedGraphMetaMoveSnapshot {
  readonly source?: GraphMeta;
  readonly destinationBefore: undefined;
  readonly destinationAfter: GraphMeta;
}

interface PreparedViewportMoveSnapshot {
  readonly before: Readonly<Record<string, EditorViewport>>;
  readonly after: Readonly<Record<string, EditorViewport>>;
}

export interface PreparedGraphResourceMove {
  readonly from: string;
  readonly to: string;
  readonly kind: 'event' | 'function';
  readonly name: string;
  readonly destinationProjection: EditorGraphProjectionDto;
  readonly destinationRequestGeneration: number;
  readonly graphProjectionPlan: PreparedGraphProjectionReplacements;
  readonly resourceSnapshot: PreparedResourceMoveSnapshot;
  readonly documentSnapshot: PreparedDocumentMoveSnapshot;
  readonly tabSnapshot: PreparedTabMoveSnapshot;
  readonly sessionSnapshot: PreparedSessionMoveSnapshot;
  readonly referenceSnapshot: PreparedGraphReferenceMoveSnapshot;
  readonly variableScopeSnapshot: PreparedVariableScopeMoveSnapshot;
  readonly graphMetaSnapshot: PreparedGraphMetaMoveSnapshot;
  readonly viewportSnapshot: PreparedViewportMoveSnapshot;
}

function assertMove(move: ResourceMoveDto, projection: EditorGraphProjectionDto): void {
  if (!move.from || !move.to || move.from === move.to
    || (move.kind !== 'event' && move.kind !== 'function')
    || !move.name.trim()) {
    throw new Error('graph resource move is malformed');
  }
  if (projection.graphPath !== move.to || projection.basis?.graphPath !== move.to
    || !Number.isSafeInteger(projection.sourceRevision) || projection.sourceRevision < 0) {
    throw new Error('destination projection identity or basis is malformed');
  }
  try {
    const entities = toProjectionEntities(projection);
    if (entities.graphPath !== move.to) {
      throw new Error('projection graph path does not match destination');
    }
  } catch {
    throw new Error('destination projection entities are malformed');
  }
}

function prepareTabs(from: string, to: string): PreparedTabMoveSnapshot {
  const before = structuredClone(useEditorTabStore.getState().snapshotMemento());
  const after = structuredClone(before);
  const tab = after.registry[from];
  if (tab) {
    after.registry[to] = { ...tab, id: to };
    delete after.registry[from];
  }
  for (const placement of Object.values(after.placements)) {
    placement.tabIds = placement.tabIds.map((id) => id === from ? to : id);
    placement.selectedTabIds = placement.selectedTabIds.map((id) => id === from ? to : id);
    if (placement.activeTabId === from) placement.activeTabId = to;
  }
  return { before, after };
}

function prepareViewport(from: string, to: string): PreparedViewportMoveSnapshot {
  const before = structuredClone(useViewportStore.getState().viewports);
  const after = structuredClone(before);
  for (const key of Object.keys(after)) {
    const scope = parseViewportScopeKey(key);
    if (!scope || scope.graphPath !== from) continue;
    const destinationKey = viewportScopeKey({ ...scope, graphPath: to });
    if (after[destinationKey]) throw new Error(`destination viewport '${destinationKey}' already exists`);
    after[destinationKey] = after[key];
    delete after[key];
  }
  return { before, after };
}

function prepareReferences(from: string, to: string): PreparedGraphReferenceMoveSnapshot {
  const normalizedFrom = normalizeGraphResourcePath(from);
  const normalizedTo = normalizeGraphResourcePath(to);
  const callers: PreparedCallerReference[] = [];
  for (const [graphPath, bucket] of Object.entries(useGraphDataStore.getState().graphEntities)) {
    for (const node of Object.values(bucket.nodes)) {
      if (isCallFunctionNodeType(node.nodeType)
        && node.subGraphPath
        && normalizeGraphResourcePath(node.subGraphPath) === normalizedFrom) {
        callers.push({
          graphPath,
          nodeId: node.id,
          before: node.subGraphPath,
          after: normalizedTo,
        });
      }
    }
  }
  return { callers };
}

function prepareVariableScopes(from: string, to: string): PreparedVariableScopeMoveSnapshot {
  const normalizedFrom = normalizeGraphResourcePath(from);
  const normalizedTo = normalizeGraphResourcePath(to);
  const installs: PreparedVariableScopeInstall[] = [];
  for (const variable of Object.values(useVariableStore.getState().variables)) {
    const scope = variable.scope;
    if (scope.type === 'event' && normalizeGraphResourcePath(scope.eventPath) === normalizedFrom) {
      installs.push({
        id: variable.id,
        before: scope,
        after: { type: 'event', eventPath: normalizedTo },
      });
    } else if (scope.type === 'function'
      && normalizeGraphResourcePath(scope.functionPath) === normalizedFrom) {
      installs.push({
        id: variable.id,
        before: scope,
        after: { type: 'function', functionPath: normalizedTo },
      });
    }
  }
  return { installs };
}

export function prepareGraphResourceMove(
  move: ResourceMoveDto,
  destinationProjection: EditorGraphProjectionDto,
): PreparedGraphResourceMove {
  assertMove(move, destinationProjection);
  const resourceState = useResourceStore.getState();
  const source = lookupGraphResource(resourceState.resources, move.from, move.kind);
  if (!source || source.id !== move.from || source.kind !== move.kind) {
    throw new Error(`missing source resource identity '${move.from}'`);
  }
  if (lookupGraphResource(resourceState.resources, move.to, move.kind)) {
    throw new Error(`destination resource '${move.to}' already exists`);
  }
  const graphState = useGraphDataStore.getState();
  if (graphState.graphEntities[move.to]) {
    throw new Error(`destination projection '${move.to}' already exists`);
  }
  const graphMeta = useGraphMetaStore.getState().graphs;
  if (graphMeta[move.to]) throw new Error(`destination graph metadata '${move.to}' already exists`);

  const fromKey = resourceKey({ id: move.from, kind: move.kind });
  const toKey = resourceKey({ id: move.to, kind: move.kind });
  const documents = useDocumentStateStore.getState().documents;
  if (documents[toKey]) throw new Error(`destination document '${move.to}' already exists`);
  const sourceDocument = documents[fromKey];
  const destinationAfterLoadedMark = buildGraphResourceMeta(move.kind, move.to, move.name, {
    loaded: true,
    hasDirtyDocument: source.hasDirtyDocument,
    hasStaleDocument: source.hasStaleDocument,
    hasConflictDocument: source.hasConflictDocument,
  });
  const destinationDocument = sourceDocument
    ? { ...sourceDocument, resourceKey: toKey }
    : undefined;
  const focused = useGraphSessionStore.getState().focusedSession;
  const sourceMeta = graphMeta[move.from];
  const preparedProjection = prepareGraphProjectionReplacements(
    [{ graphPath: move.to, projection: destinationProjection }],
    graphState.graphEntities,
  );
  if (!preparedProjection.prepared) {
    throw new Error(`destination projection '${move.to}' could not be prepared`);
  }
  const graphEntitiesAfter = { ...preparedProjection.plan.graphEntities };
  delete graphEntitiesAfter[move.from];

  return Object.freeze({
    from: move.from,
    to: move.to,
    kind: move.kind,
    name: move.name,
    destinationProjection,
    destinationRequestGeneration: (graphState.graphEntities[move.to]?.requestGeneration ?? 0) + 1,
    graphProjectionPlan: Object.freeze({
      graphPaths: preparedProjection.plan.graphPaths,
      graphEntities: graphEntitiesAfter,
    }),
    resourceSnapshot: Object.freeze({
      fromKey,
      toKey,
      source: structuredClone(source),
      destinationBefore: undefined,
      destinationBeforeLoadedMark: { ...destinationAfterLoadedMark, loaded: false },
      destinationAfterLoadedMark,
      graphOrderAfter: resourceState.graphOrder.map((path) => path === move.from ? move.to : path),
    }),
    documentSnapshot: Object.freeze({
      fromKey,
      toKey,
      source: sourceDocument ? structuredClone(sourceDocument) : undefined,
      destinationBefore: undefined,
      destinationBeforeLoadedMark: destinationDocument
        ? { ...destinationDocument, loaded: false }
        : undefined,
      destinationAfterLoadedMark: destinationDocument
        ? { ...destinationDocument, loaded: true }
        : undefined,
    }),
    tabSnapshot: Object.freeze(prepareTabs(move.from, move.to)),
    sessionSnapshot: Object.freeze({
      before: focused ? structuredClone(focused) : null,
      after: focused?.graphPath === move.from ? { ...focused, graphPath: move.to } : focused,
    }),
    referenceSnapshot: Object.freeze(prepareReferences(move.from, move.to)),
    variableScopeSnapshot: Object.freeze(prepareVariableScopes(move.from, move.to)),
    graphMetaSnapshot: Object.freeze({
      source: sourceMeta ? structuredClone(sourceMeta) : undefined,
      destinationBefore: undefined,
      destinationAfter: {
        ...(sourceMeta ?? { path: move.to, type: move.kind }),
        path: move.to,
        name: move.name,
        type: move.kind,
      },
    }),
    viewportSnapshot: Object.freeze(prepareViewport(move.from, move.to)),
  });
}

function commitResourceSnapshot(plan: PreparedGraphResourceMove): void {
  useResourceStore.setState((state) => {
    const resources = { ...state.resources };
    delete resources[plan.resourceSnapshot.fromKey];
    resources[plan.resourceSnapshot.toKey] = plan.resourceSnapshot.destinationBeforeLoadedMark;
    return { resources, graphOrder: [...plan.resourceSnapshot.graphOrderAfter] };
  });
}

function commitDocumentSnapshot(plan: PreparedGraphResourceMove): void {
  useDocumentStateStore.setState((state) => {
    const documents = { ...state.documents };
    delete documents[plan.documentSnapshot.fromKey];
    if (plan.documentSnapshot.destinationBeforeLoadedMark) {
      documents[plan.documentSnapshot.toKey] = plan.documentSnapshot.destinationBeforeLoadedMark;
    }
    return { documents };
  });
}

function commitGraphMetaSnapshot(plan: PreparedGraphResourceMove): void {
  useGraphMetaStore.setState((state) => {
    const graphs = { ...state.graphs };
    delete graphs[plan.from];
    graphs[plan.to] = plan.graphMetaSnapshot.destinationAfter;
    return { graphs };
  });
}

function commitReferenceSnapshot(plan: PreparedGraphResourceMove): void {
  if (plan.referenceSnapshot.callers.length === 0) return;
  useGraphDataStore.setState((state) => {
    const graphEntities = { ...state.graphEntities };
    for (const install of plan.referenceSnapshot.callers) {
      const bucket = graphEntities[install.graphPath];
      const node = bucket?.nodes[install.nodeId];
      if (!bucket || !node || node.subGraphPath !== install.before) continue;
      graphEntities[install.graphPath] = {
        ...bucket,
        nodes: {
          ...bucket.nodes,
          [install.nodeId]: { ...node, subGraphPath: install.after },
        },
      };
    }
    return { graphEntities };
  });
}

function commitVariableScopes(plan: PreparedGraphResourceMove): void {
  if (plan.variableScopeSnapshot.installs.length === 0) return;
  useVariableStore.setState((state) => {
    const variables = { ...state.variables };
    for (const install of plan.variableScopeSnapshot.installs) {
      const variable = variables[install.id];
      if (variable) variables[install.id] = { ...variable, scope: install.after };
    }
    return { variables };
  });
}

function commitLoadedMark(plan: PreparedGraphResourceMove): void {
  useResourceStore.setState((state) => ({
    resources: {
      ...state.resources,
      [plan.resourceSnapshot.toKey]: plan.resourceSnapshot.destinationAfterLoadedMark,
    },
  }));
  if (plan.documentSnapshot.destinationAfterLoadedMark) {
    useDocumentStateStore.setState((state) => ({
      documents: {
        ...state.documents,
        [plan.documentSnapshot.toKey]: plan.documentSnapshot.destinationAfterLoadedMark as DocumentState,
      },
    }));
  }
}

export function commitGraphResourceMoveOwnership(plan: PreparedGraphResourceMove): void {
  commitResourceSnapshot(plan);
  commitDocumentSnapshot(plan);
  commitGraphMetaSnapshot(plan);
  commitReferenceSnapshot(plan);
  commitVariableScopes(plan);
  useGraphSessionStore.setState({ focusedSession: plan.sessionSnapshot.after });
  useEditorTabStore.getState().applyMemento(plan.tabSnapshot.after);
  useViewportStore.setState({ viewports: { ...plan.viewportSnapshot.after } });
  commitLoadedMark(plan);
}

export function commitGraphResourceMove(plan: PreparedGraphResourceMove): void {
  commitPreparedGraphProjectionReplacements(plan.graphProjectionPlan);
  commitGraphResourceMoveOwnership(plan);
}
