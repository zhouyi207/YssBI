import { beforeEach, describe, expect, it } from 'vitest';
import { useDocumentStateStore, useResourceStore, buildGraphResourceMeta, type ProjectResourceMeta } from '@/features/core/resource';
import {
  reconcileResourceSnapshot,
  applySnapshotDocumentPatches,
} from './resourceSnapshotReconcile';
import { selectFirstGraphResource, selectGraphResourcesByKind } from './resourceSelectors';
import { resourceKey } from './resourceTypes';

function graphResource(
  id: string,
  kind: 'event' | 'function',
  name: string,
): ProjectResourceMeta {
  return buildGraphResourceMeta(kind, id, name);
}

describe('resource snapshot reconcile', () => {
  beforeEach(() => {
    useResourceStore.getState().clear();
    useDocumentStateStore.getState().clear();
  });

  it('marks loaded clean resources stale when snapshot metadata changes', () => {
    const previous = graphResource('g1', 'event', 'Old Name');
    previous.loaded = true;
    useDocumentStateStore.getState().upsertDocument({
      resourceKey: resourceKey(previous),
      loaded: true,
      dirty: false,
      stale: false,
      missing: false,
      conflict: false,
      version: 1,
    });

    const incoming = [graphResource('g1', 'event', 'New Name')];
    const { resources, documentPatches } = reconcileResourceSnapshot(
      incoming,
      { [resourceKey(previous)]: previous },
    );
    applySnapshotDocumentPatches(documentPatches);

    expect(resources[0]).toMatchObject({
      loaded: true,
      hasStaleDocument: true,
      hasConflictDocument: false,
    });
    expect(useDocumentStateStore.getState().documents[resourceKey(previous)]?.stale).toBe(true);
  });

  it('retains loaded untitled drafts without marking missing', () => {
    const previous = buildGraphResourceMeta('event', 'untitled:event:Untitled-1', 'Draft');
    previous.loaded = true;
    useDocumentStateStore.getState().upsertDocument({
      resourceKey: resourceKey(previous),
      loaded: true,
      dirty: false,
      stale: false,
      missing: false,
      conflict: false,
      version: 1,
    });

    const { resources, documentPatches } = reconcileResourceSnapshot([], {
      [resourceKey(previous)]: previous,
    });
    applySnapshotDocumentPatches(documentPatches);

    expect(resources).toHaveLength(1);
    expect(resources[0]).toMatchObject({
      id: 'untitled:event:Untitled-1',
      exists: false,
      loaded: true,
    });
    expect(documentPatches).toHaveLength(0);
    expect(useDocumentStateStore.getState().documents[resourceKey(previous)]?.missing).toBe(false);
  });

  it('retains missing loaded resources absent from the snapshot', () => {
    const previous = graphResource('g1', 'event', 'Removed');
    previous.loaded = true;
    useDocumentStateStore.getState().upsertDocument({
      resourceKey: resourceKey(previous),
      loaded: true,
      dirty: false,
      stale: false,
      missing: false,
      conflict: false,
      version: 1,
    });

    const { resources, documentPatches } = reconcileResourceSnapshot([], {
      [resourceKey(previous)]: previous,
    });
    applySnapshotDocumentPatches(documentPatches);

    expect(resources).toHaveLength(1);
    expect(resources[0]).toMatchObject({
      id: 'g1',
      exists: false,
      loaded: true,
    });
    expect(useDocumentStateStore.getState().documents[resourceKey(previous)]?.missing).toBe(true);
  });
});

describe('resource selectors', () => {
  it('derives event/function lists and first graph from ResourceStore', () => {
    useResourceStore.getState().setSnapshot({
      resources: [
        graphResource('e1', 'event', 'Event A'),
        graphResource('f1', 'function', 'Function A'),
      ],
      graphOrder: ['e1', 'f1'],
    });

    const resources = useResourceStore.getState().resources;
    expect(selectGraphResourcesByKind(resources, 'event')).toEqual({
      e1: { id: 'e1', name: 'Event A' },
    });
    expect(selectFirstGraphResource(resources, ['e1', 'f1'])?.id).toBe('e1');
  });
});
