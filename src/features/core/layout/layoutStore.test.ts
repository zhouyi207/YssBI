import { beforeEach, describe, expect, it } from 'vitest';
import { useLayoutStore } from './layoutStore';

describe('layoutStore tab selection lifecycle', () => {
  beforeEach(() => {
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
          data: {
            component: 'GraphEditor',
            tabs: [
              { id: 'g1', component: 'GraphEditor', type: 'event' },
              { id: 'g2', component: 'GraphEditor', type: 'event' },
            ],
            activeTabId: 'g1',
            params: { selectedNodeIds: ['node-from-g1'] },
          },
        },
      },
      activeEditorGroupId: 'editor',
    });
  });

  it('clears stale selected node ids when closing the active tab selects another tab', () => {
    useLayoutStore.getState().removeTab('editor', 'g1');

    const editor = useLayoutStore.getState().nodes.editor;
    expect(editor.data?.activeTabId).toBe('g2');
    expect(editor.data?.params?.selectedNodeIds).toEqual([]);
  });

  it('clears stale selected node ids when activating an existing tab', () => {
    useLayoutStore.getState().addTab('editor', {
      id: 'g2',
      component: 'GraphEditor',
      type: 'event',
    });

    const editor = useLayoutStore.getState().nodes.editor;
    expect(editor.data?.activeTabId).toBe('g2');
    expect(editor.data?.params?.selectedNodeIds).toEqual([]);
  });

  it('keeps selected node ids when activating the already active tab', () => {
    useLayoutStore.getState().addTab('editor', {
      id: 'g1',
      component: 'GraphEditor',
      type: 'event',
    });

    const editor = useLayoutStore.getState().nodes.editor;
    expect(editor.data?.activeTabId).toBe('g1');
    expect(editor.data?.params?.selectedNodeIds).toEqual(['node-from-g1']);
  });
});
