import { useGraphDataStore } from '@/features/core/dataStore';
import type { GraphEntityBucket } from '@/features/core/dataStore/graphEntityAccess';
import { useGraphSessionStore } from '@/features/core/graphSession/graphSessionStore';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
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
  useGraphSessionStore.getState().remapActivePaths(from, to);

  const layoutStore = useLayoutStore.getState();
  for (const [nodeId, node] of Object.entries(layoutStore.nodes)) {
    const tabs = node.data?.tabs;
    if (!tabs?.some((tab) => tab.id === from)) continue;
    layoutStore.updateNode(nodeId, {
      data: {
        ...node.data,
        activeTabId: node.data?.activeTabId === from ? to : node.data?.activeTabId,
        tabs: tabs.map((tab) => (tab.id === from ? { ...tab, id: to } : tab)),
      },
    });
  }
}
