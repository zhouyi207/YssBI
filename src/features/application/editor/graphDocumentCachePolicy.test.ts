import { beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('@/services/graph/graphService', () => ({
  GraphService: {
    unloadProjectGraph: vi.fn().mockResolvedValue(undefined),
  },
}));

import {
  enforceGraphDocumentCacheLimit,
  MAX_HYDRATED_GRAPH_DOCUMENTS,
  touchGraphDocument,
} from './graphDocumentCachePolicy';
import { useGraphDataStore } from '@/features/core/dataStore';
import { useGraphSessionStore } from '@/features/core/graphSession/graphSessionStore';

describe('graphDocumentCachePolicy', () => {
  beforeEach(() => {
    useGraphSessionStore.setState({ focusedSession: null });
    useGraphDataStore.setState({ graphEntities: {} });
  });

  it('evicts LRU graphs beyond the hydrated cap', async () => {
    useGraphSessionStore.setState({
      focusedSession: { groupId: 'g1', graphPath: 'active-graph' },
    });
    touchGraphDocument('ancient-graph');
    touchGraphDocument('oldest-graph');
    touchGraphDocument('older-graph');
    touchGraphDocument('old-graph');
    touchGraphDocument('active-graph');

    useGraphDataStore.setState({
      graphEntities: {
        'active-graph': {} as never,
        'old-graph': {} as never,
        'older-graph': {} as never,
        'oldest-graph': {} as never,
        'ancient-graph': {} as never,
      },
    });

    await enforceGraphDocumentCacheLimit();

    const remaining = Object.keys(useGraphDataStore.getState().graphEntities);
    expect(remaining).toContain('active-graph');
    expect(remaining.length).toBeLessThanOrEqual(MAX_HYDRATED_GRAPH_DOCUMENTS);
    expect(remaining).not.toContain('old-graph');
  });
});
