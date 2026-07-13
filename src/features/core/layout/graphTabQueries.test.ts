import { beforeEach, describe, expect, it } from 'vitest';
import { useLayoutStore } from './layoutStore';
import { isGraphOpenInAnyTab } from './graphTabQueries';
import { resetEditorTabStore, seedEditorGroupTabs } from './editorTabTestUtils';

describe('graphTabQueries', () => {
  beforeEach(() => {
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
      { id: 'events/A.yssbi-event', component: 'GraphEditor', type: 'event' },
      { id: 'functions/B.yssbi-function', component: 'GraphEditor', type: 'function' },
    ], 'events/A.yssbi-event');
  });

  it('isGraphOpenInAnyTab returns true for paths attached to editor tabs', () => {
    expect(isGraphOpenInAnyTab('events/A.yssbi-event')).toBe(true);
    expect(isGraphOpenInAnyTab('functions/B.yssbi-function')).toBe(true);
  });

  it('isGraphOpenInAnyTab returns false for paths not in any tab', () => {
    expect(isGraphOpenInAnyTab('events/Missing.yssbi-event')).toBe(false);
  });
});
