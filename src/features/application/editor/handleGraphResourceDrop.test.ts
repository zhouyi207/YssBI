import { beforeEach, describe, expect, it, vi } from 'vitest';
import { handleGraphResourceDrop } from './handleGraphResourceDrop';
import { openGraphInEditor } from './openGraphInEditor';
import { DROP_TYPES } from '@/features/core/dnd';

vi.mock('./openGraphInEditor', () => ({
  openGraphInEditor: vi.fn().mockResolvedValue(undefined),
}));

describe('handleGraphResourceDrop', () => {
  beforeEach(() => {
    vi.mocked(openGraphInEditor).mockClear();
  });

  const resource = { id: 'evt-1', name: 'Main', type: 'event' as const };

  it('opens pinned graph at TabBar insert index', async () => {
    await handleGraphResourceDrop(resource, {
      dropType: DROP_TYPES.TABBAR,
      targetNodeId: 'editor-a',
      targetTabIndex: 2,
    });

    expect(openGraphInEditor).toHaveBeenCalledWith('evt-1', 'Main', 'event', 'editor-a', {
      pinned: true,
      insertIndex: 2,
    });
  });

  it('opens pinned graph on canvas drop', async () => {
    await handleGraphResourceDrop(resource, {
      dropType: DROP_TYPES.CANVAS,
      groupId: 'editor-b',
    });

    expect(openGraphInEditor).toHaveBeenCalledWith('evt-1', 'Main', 'event', 'editor-b', {
      pinned: true,
    });
  });
});
