import { useDocumentStateStore, type DocumentState } from './documentStateStore';
import type { ProjectResourceMeta, ResourceKey } from './resourceTypes';
import { resourceKey } from './resourceTypes';

export interface ResourceSnapshot {
  resources: ProjectResourceMeta[];
  graphOrder: string[];
}

function snapshotMetaFingerprint(resource: ProjectResourceMeta): string {
  return `${resource.name}\0${resource.uri}`;
}

export interface SnapshotReconcileResult {
  resources: ProjectResourceMeta[];
  documentPatches: Array<{ key: ResourceKey; patch: Partial<DocumentState> }>;
}

/**
 * Reconcile a backend index snapshot with open document state.
 * Loaded persisted resources absent from the snapshot are retained as missing entries.
 */
export function reconcileResourceSnapshot(
  incoming: ProjectResourceMeta[],
  previousByKey: Record<ResourceKey, ProjectResourceMeta>,
): SnapshotReconcileResult {
  const documents = useDocumentStateStore.getState().documents;
  const incomingByKey = new Map(incoming.map((resource) => [resourceKey(resource), resource]));
  const documentPatches: SnapshotReconcileResult['documentPatches'] = [];
  const resources: ProjectResourceMeta[] = [];

  for (const resource of incoming) {
    const key = resourceKey(resource);
    const previous = previousByKey[key];
    const doc = documents[key];

    const loaded = doc?.loaded ?? previous?.loaded ?? resource.loaded;
    let hasDirtyDocument = doc?.dirty ?? previous?.hasDirtyDocument ?? false;
    let hasStaleDocument = doc?.stale ?? false;
    let hasConflictDocument = doc?.conflict ?? false;

    if (previous && doc?.loaded && !doc.missing) {
      const metaChanged = snapshotMetaFingerprint(previous) !== snapshotMetaFingerprint(resource);
      if (metaChanged) {
        if (doc.dirty) {
          hasConflictDocument = true;
          hasStaleDocument = false;
          documentPatches.push({ key, patch: { conflict: true, stale: false, missing: false } });
        } else {
          hasStaleDocument = true;
          hasConflictDocument = false;
          documentPatches.push({ key, patch: { stale: true, conflict: false, missing: false } });
        }
      }
    }

    resources.push({
      ...resource,
      loaded,
      exists: true,
      hasDirtyDocument,
      hasStaleDocument,
      hasConflictDocument,
    });
  }

  for (const [key, previous] of Object.entries(previousByKey) as Array<[ResourceKey, ProjectResourceMeta]>) {
    if (incomingByKey.has(key)) continue;
    const doc = documents[key];
    if (!doc?.loaded && !previous.loaded) continue;

    documentPatches.push({
      key,
      patch: {
        missing: true,
        stale: false,
        conflict: doc?.dirty ?? previous.hasConflictDocument,
      },
    });

    resources.push({
      ...previous,
      exists: false,
      loaded: true,
      hasDirtyDocument: doc?.dirty ?? previous.hasDirtyDocument,
      hasStaleDocument: false,
      hasConflictDocument: doc?.dirty ?? previous.hasConflictDocument,
    });
  }

  return { resources, documentPatches };
}

export function applySnapshotDocumentPatches(
  patches: SnapshotReconcileResult['documentPatches'],
): void {
  const store = useDocumentStateStore.getState();
  for (const { key, patch } of patches) {
    const previous = store.documents[key];
    if (!previous) {
      store.upsertDocument({
        resourceKey: key,
        loaded: true,
        dirty: patch.conflict ?? false,
        stale: patch.stale ?? false,
        missing: patch.missing ?? false,
        conflict: patch.conflict ?? false,
        version: 0,
      });
      continue;
    }
    store.patchDocument(key, patch);
  }
}
