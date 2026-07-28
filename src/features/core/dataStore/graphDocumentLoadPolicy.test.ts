import { describe, expect, it, beforeEach } from 'vitest';
import { useGraphDataStore } from './graphDataStore';
import { useDocumentStateStore } from '@/features/core/resource/documentStateStore';
import { markResourceLoaded, resourceKey } from '@/features/core/resource';
import { isGraphCachedInMemory } from './graphDocumentLoadPolicy';
import { makeEditorProjectionFixture } from '@/tests/helpers/editorProjectionFixtures';

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
    const fixture = makeEditorProjectionFixture({ graphPath: 'evt-1' });
    useGraphDataStore.getState().replaceProjection('evt-1', fixture.projection, 1);
    expect(isGraphCachedInMemory('evt-1')).toBe(false);
  });

  it('returns false when a bucket exists for an unloaded graph resource', () => {
    const fixture = makeEditorProjectionFixture({ graphPath });
    useGraphDataStore.getState().replaceProjection(graphPath, fixture.projection, 1);

    expect(isGraphCachedInMemory(graphPath)).toBe(false);
  });

  it('returns true when graph is cached and document is clean', () => {
    const fixture = makeEditorProjectionFixture({ graphPath });
    useGraphDataStore.getState().replaceProjection(graphPath, fixture.projection, 1);
    markResourceLoaded({ id: graphPath, kind: 'event' });

    expect(isGraphCachedInMemory(graphPath)).toBe(true);
  });

  it('returns false when graph is stale', () => {
    const fixture = makeEditorProjectionFixture({ graphPath });
    useGraphDataStore.getState().replaceProjection(graphPath, fixture.projection, 1);
    markResourceLoaded({ id: graphPath, kind: 'event' });
    useDocumentStateStore.getState().patchDocument(docKey, { stale: true });

    expect(isGraphCachedInMemory(graphPath)).toBe(false);
  });
});
