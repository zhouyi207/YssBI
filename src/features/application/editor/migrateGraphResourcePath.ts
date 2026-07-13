import { useGraphDataStore } from '@/features/core/dataStore';
import type { GraphEntityBucket } from '@/features/core/dataStore/graphEntityAccess';
import { useGraphSessionStore } from '@/features/core/graphSession/graphSessionStore';
import { useEditorTabStore } from '@/features/core/layout/editorTabStore';
import {
  buildGraphResourceMeta,
  lookupGraphResource,
  migrateDocumentStatePath,
  useResourceStore,
} from '@/features/core/resource';
import type { ResourceKind } from '@/features/core/resource/resourceTypes';
import { cascadeGraphPathReferences } from './cascadeGraphPathReferences';

function remapGraphEntityPath(from: string, to: string): void {
  useGraphDataStore.setState((state) => {
    const bucket = state.graphEntities[from];
    if (!bucket) return state;
    const nextBucket: GraphEntityBucket = {
      ...bucket,
      nodes: Object.fromEntries(
        Object.entries(bucket.nodes).map(([id, node]) => [id, { ...node, graphPath: to }]),
      ),
    };
    const graphEntities = { ...state.graphEntities };
    delete graphEntities[from];
    graphEntities[to] = nextBucket;
    return { graphEntities };
  });
}

/** Migrate frontend stores when a graph resource path changes on disk. */
export function migrateGraphResourcePath(
  from: string,
  to: string,
  kind: Extract<ResourceKind, 'event' | 'function'>,
): void {
  if (from === to) return;

  const resourceStore = useResourceStore.getState();
  const oldMeta = lookupGraphResource(resourceStore.resources, from, kind);
  if (oldMeta) {
    resourceStore.removeResource({ id: from, kind });
    resourceStore.upsertResource(
      buildGraphResourceMeta(kind, to, oldMeta.name, {
        loaded: oldMeta.loaded,
        hasDirtyDocument: oldMeta.hasDirtyDocument,
        hasStaleDocument: oldMeta.hasStaleDocument,
        hasConflictDocument: oldMeta.hasConflictDocument,
      }),
    );
  }

  remapGraphEntityPath(from, to);
  cascadeGraphPathReferences(from, to);
  migrateDocumentStatePath(from, to, kind);
  useGraphSessionStore.getState().remapFocusedGraphPath(from, to);
  useEditorTabStore.getState().renameTabId(from, to);
}
