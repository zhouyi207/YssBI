import { beforeEach, describe, expect, it } from 'vitest';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { resetEditorTabStore, seedEditorGroupTabs } from '@/features/core/layout/editorTabTestUtils';
import { useGraphSessionStore } from '@/features/core/graphSession/graphSessionStore';
import { shouldRetainGraphDocument } from './graphDocumentRetention';

describe('graphDocumentRetention', () => {
  beforeEach(() => {
    useGraphSessionStore.getState().reset();
    resetEditorTabStore();
    useLayoutStore.setState({
      rootId: 'root',
      nodes: {
        root: {
          id: 'root',
          type: 'row',
          parentId: null,
          children: ['editor'],
        },
        editor: {
          id: 'editor',
          type: 'component',
          parentId: 'root',
          data: { component: 'GraphEditor' },
        },
      },
    });
    seedEditorGroupTabs('editor', [
      { id: 'events/open.yssbi-event', component: 'GraphEditor', type: 'event' },
    ]);
  });

  it('retains focused graph paths', () => {
    useGraphSessionStore.getState().setFocusedSession('editor', 'events/focused.yssbi-event');

    expect(shouldRetainGraphDocument('events/focused.yssbi-event')).toBe(true);
  });

  it('retains graphs that remain open in editor tabs', () => {
    expect(shouldRetainGraphDocument('events/open.yssbi-event')).toBe(true);
  });

  it('does not retain graphs with no tab, session, or dirty state', () => {
    expect(shouldRetainGraphDocument('events/closed.yssbi-event')).toBe(false);
  });
});
