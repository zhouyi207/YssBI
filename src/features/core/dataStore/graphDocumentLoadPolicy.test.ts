import { describe, expect, it, beforeEach } from 'vitest';
import { useGraphDataStore } from './graphDataStore';
import { useDocumentStateStore } from '@/features/core/resource/documentStateStore';
import { markResourceLoaded, resourceKey } from '@/features/core/resource';
import { isGraphCachedInMemory } from './graphDocumentLoadPolicy';
import { makeTestGraph } from '@/tests/helpers/graphFixtures';

describe('graphDocumentLoadPolicy', () => {
  const graphPath = 'events/Main.yssbi-event';
  const docKey = resourceKey({ id: graphPath, kind: 'event' });

  beforeEach(() => {
    useGraphDataStore.setState({ graphEntities: {} });
    useDocumentStateStore.getState().clear();
  });

  it('returns false when graph is not in memory', () => {
    expect(isGraphCachedInMemory('events/Missing.yssbi-event')).toBe(false);
  });

  it('returns false when path kind cannot be inferred', () => {
    useGraphDataStore.getState().addGraphFromData('evt-1', makeTestGraph({ path: 'evt-1' }));
    expect(isGraphCachedInMemory('evt-1')).toBe(false);
  });

  it('returns true when graph is cached and document is clean', () => {
    useGraphDataStore.getState().addGraphFromData(graphPath, makeTestGraph({ path: graphPath }));
    markResourceLoaded({ id: graphPath, kind: 'event' });

    expect(isGraphCachedInMemory(graphPath)).toBe(true);
  });

  it('returns false when graph is stale', () => {
    useGraphDataStore.getState().addGraphFromData(graphPath, makeTestGraph({ path: graphPath }));
    markResourceLoaded({ id: graphPath, kind: 'event' });
    useDocumentStateStore.getState().patchDocument(docKey, { stale: true });

    expect(isGraphCachedInMemory(graphPath)).toBe(false);
  });
});
