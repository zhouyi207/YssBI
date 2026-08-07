import type { ResourceMoveDto } from '@/shared/types/dto/editorMutation';
import { useGraphDataStore, useGraphMetaStore } from '@/features/core/dataStore';
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
  readonly destinationAfter: ProjectResourceMeta;
  readonly graphOrderAfter: readonly string[];
}

export interface PreparedDocumentMoveSnapshot {
  readonly fromKey: ResourceKey;
  readonly toKey: ResourceKey;
  readonly source?: DocumentState;
  readonly destinationBefore: undefined;
  readonly destinationAfter?: DocumentState;
}

export interface PreparedTabMoveSnapshot {
  readonly before: EditorTabMemento;
  readonly after: EditorTabMemento;
}

export interface PreparedSessionMoveSnapshot {
  readonly before: FocusedGraphSession | null;
  readonly after: FocusedGraphSession | null;
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
  readonly hasAuthoritativeDestinationReplacement: boolean;
  readonly resourceSnapshot: PreparedResourceMoveSnapshot;
  readonly documentSnapshot: PreparedDocumentMoveSnapshot;
  readonly tabSnapshot: PreparedTabMoveSnapshot;
  readonly sessionSnapshot: PreparedSessionMoveSnapshot;

  readonly graphMetaSnapshot: PreparedGraphMetaMoveSnapshot;
  readonly viewportSnapshot: PreparedViewportMoveSnapshot;
}

function assertMove(move: ResourceMoveDto): void {
  if (!move.from || !move.to || move.from === move.to
    || (move.kind !== 'event' && move.kind !== 'function')
    || !move.name.trim()) {
    throw new Error('graph resource move is malformed');
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


export function prepareGraphResourceMove(
  move: ResourceMoveDto,
  hasAuthoritativeDestinationReplacement: boolean,
): PreparedGraphResourceMove {
  assertMove(move);
  const resourceState = useResourceStore.getState();
  const source = lookupGraphResource(resourceState.resources, move.from, move.kind);
  if (!source || source.id !== move.from || source.kind !== move.kind) {
    throw new Error(`missing source resource identity '${move.from}'`);
  }
  if (lookupGraphResource(resourceState.resources, move.to, move.kind)) {
    throw new Error(`destination resource '${move.to}' already exists`);
  }
  if (source.loaded !== hasAuthoritativeDestinationReplacement) {
    throw new Error(`move destination replacement disagrees with source loaded ownership '${move.from}'`);
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
  const destinationAfter = buildGraphResourceMeta(move.kind, move.to, move.name, {
    revision: source.revision,
    loaded: source.loaded,
    hasDirtyDocument: source.hasDirtyDocument,
    hasStaleDocument: source.hasStaleDocument,
    hasConflictDocument: source.hasConflictDocument,
  });
  const destinationDocument = sourceDocument
    ? { ...sourceDocument, resourceKey: toKey, loaded: sourceDocument.loaded }
    : undefined;
  const focused = useGraphSessionStore.getState().focusedSession;
  const sourceMeta = graphMeta[move.from];

  return Object.freeze({
    from: move.from,
    to: move.to,
    kind: move.kind,
    name: move.name,
    hasAuthoritativeDestinationReplacement,
    resourceSnapshot: Object.freeze({
      fromKey,
      toKey,
      source: structuredClone(source),
      destinationBefore: undefined,
      destinationAfter,
      graphOrderAfter: resourceState.graphOrder.map((path) => path === move.from ? move.to : path),
    }),
    documentSnapshot: Object.freeze({
      fromKey,
      toKey,
      source: sourceDocument ? structuredClone(sourceDocument) : undefined,
      destinationBefore: undefined,
      destinationAfter: destinationDocument,
    }),
    tabSnapshot: Object.freeze(prepareTabs(move.from, move.to)),
    sessionSnapshot: Object.freeze({
      before: focused ? structuredClone(focused) : null,
      after: focused?.graphPath === move.from ? { ...focused, graphPath: move.to } : focused,
    }),

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
