import { beforeEach, describe, expect, it, vi } from 'vitest';
import { handleGraphResourceDrop } from './handleGraphResourceDrop';
import { openGraphInEditor } from './openGraphInEditor';
import { EditorGroupsService } from '@/features/core/layout/editorGroupsService';

vi.mock('./openGraphInEditor', () => ({
  openGraphInEditor: vi.fn().mockResolvedValue(undefined),
}));

vi.mock('./switchEditorTab', () => ({
  switchEditorTab: vi.fn().mockResolvedValue(true),
}));

vi.mock('@/features/core/layout/editorGroupsService', () => ({
  EditorGroupsService: {
    splitGroupAtEdge: vi.fn(),
  },
}));

describe('handleGraphResourceDrop', () => {
  beforeEach(() => {
    vi.mocked(openGraphInEditor).mockClear();
    vi.mocked(EditorGroupsService.splitGroupAtEdge).mockReset();
  });

  const resource = { id: 'evt-1', name: 'Main', type: 'event' as const };

  it('opens pinned graph at TabBar insert index', async () => {
    await handleGraphResourceDrop(resource, 'editor-a', { insertIndex: 2 });

    expect(openGraphInEditor).toHaveBeenCalledWith('evt-1', 'Main', 'event', 'editor-a', {
      pinned: true,
      insertIndex: 2,
    });
  });

  it('opens pinned graph on merge drop', async () => {
    await handleGraphResourceDrop(resource, 'editor-b');

    expect(openGraphInEditor).toHaveBeenCalledWith('evt-1', 'Main', 'event', 'editor-b', {
      pinned: true,
    });
  });

  it('opens a pinned Function graph on merge drop', async () => {
    const functionResource = {
      id: 'functions/Revenue.yssbi-function',
      name: 'Revenue',
      type: 'function' as const,
    };

    await handleGraphResourceDrop(functionResource, 'editor-b');

    expect(openGraphInEditor).toHaveBeenCalledWith(
      functionResource.id,
      functionResource.name,
      'function',
      'editor-b',
      { pinned: true },
    );
  });

  it('splits editor group when dropping on a split zone', async () => {
    vi.mocked(EditorGroupsService.splitGroupAtEdge).mockReturnValue('editor-new');

    await handleGraphResourceDrop(resource, 'editor-b', { edge: 'right' });

    expect(EditorGroupsService.splitGroupAtEdge).toHaveBeenCalledWith('editor-b', 'right', {
      component: 'GraphEditor',
      tabs: [],
    });
    expect(openGraphInEditor).toHaveBeenCalledWith('evt-1', 'Main', 'event', 'editor-new', {
      pinned: true,
    });
  });
});
