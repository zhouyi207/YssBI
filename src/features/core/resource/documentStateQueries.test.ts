import { beforeEach, describe, expect, it } from 'vitest';
import {
  clearResourceDocumentState,
  markResourceDirty,
  markResourceLoaded,
  migrateDocumentStatePath,
  useDocumentStateStore,
  useResourceStore,
  buildGraphResourceMeta,
  isGraphResourceDirty,
  resourceKey,
} from '@/features/core/resource';
import { collectDirtyGraphTabs } from '@/features/core/layout/tabDirty';

describe('document state queries', () => {
  beforeEach(() => {
    useDocumentStateStore.getState().clear();
    useResourceStore.getState().clear();
  });

  it('tracks dirty via DocumentState as single source of truth', () => {
    const meta = buildGraphResourceMeta('event', 'events/A.yssbi-event', 'A');
    useResourceStore.getState().upsertResource(meta);
    markResourceLoaded({ id: meta.id, kind: 'event' });

    expect(isGraphResourceDirty(meta.id, 'event')).toBe(false);
    markResourceDirty({ id: meta.id, kind: 'event' }, true);
    expect(isGraphResourceDirty(meta.id, 'event')).toBe(true);
    expect(useResourceStore.getState().resources[resourceKey(meta)]?.hasDirtyDocument).toBe(true);
  });

  it('removes untitled draft resource meta on clear', () => {
    const draft = buildGraphResourceMeta('event', 'untitled:event:Untitled-1', 'Draft');
    useResourceStore.getState().upsertResource(draft);
    markResourceLoaded({ id: draft.id, kind: 'event' });

    clearResourceDocumentState({ id: draft.id, kind: 'event' });

    expect(useResourceStore.getState().resources[resourceKey(draft)]).toBeUndefined();
    expect(
      useDocumentStateStore.getState().documents[resourceKey({ id: draft.id, kind: 'event' })],
    ).toBeUndefined();
  });

  it('migrates document state when graph path changes', () => {
    const from = 'events/Old.yssbi-event';
    const to = 'events/New.yssbi-event';
    useResourceStore.getState().upsertResource(buildGraphResourceMeta('event', from, 'Old'));
    markResourceLoaded({ id: from, kind: 'event' });
    markResourceDirty({ id: from, kind: 'event' }, true);

    migrateDocumentStatePath(from, to, 'event');

    expect(useDocumentStateStore.getState().documents[resourceKey({ id: from, kind: 'event' })]).toBeUndefined();
    expect(useDocumentStateStore.getState().documents[resourceKey({ id: to, kind: 'event' })]).toMatchObject({
      dirty: true,
      loaded: true,
    });
  });
});

describe('collectDirtyGraphTabs', () => {
  beforeEach(() => {
    useDocumentStateStore.getState().clear();
    useResourceStore.getState().clear();
  });

  it('reads dirty from DocumentState not LayoutTab.isDirty', () => {
    const path = 'events/A.yssbi-event';
    useResourceStore.getState().upsertResource(buildGraphResourceMeta('event', path, 'A'));
    markResourceDirty({ id: path, kind: 'event' }, true);

    expect(collectDirtyGraphTabs()).toEqual([]);
  });
});
