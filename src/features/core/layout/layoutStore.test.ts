import { beforeEach, describe, expect, it } from 'vitest';
import { useLayoutStore } from './layoutStore';
import {
  createInitialWorkbenchNodes,
  DEFAULT_EDITOR_GROUP_ID,
  EDITOR_AREA_ID,
} from './workbenchLayoutDefaults';

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

describe('layoutStore editor group mutations', () => {
  it('removes the last tab through the editor-grid boundary without touching chrome', () => {
    useLayoutStore.setState({
      nodes: createInitialWorkbenchNodes(),
      activeEditorGroupId: DEFAULT_EDITOR_GROUP_ID,
    });
    const store = useLayoutStore.getState();
    store.addTab(DEFAULT_EDITOR_GROUP_ID, {
      id: 'g1',
      component: 'GraphEditor',
      type: 'event',
    });
    const created = store.splitEditorGroupAtEdge(DEFAULT_EDITOR_GROUP_ID, 'right', {
      component: 'GraphEditor',
      tabs: [{ id: 'g2', component: 'GraphEditor', type: 'event' }],
      activeTabId: 'g2',
    });
    expect(created).toBeTruthy();

    useLayoutStore.setState((state) => {
      state.nodes.sidebar!.pixelSize = 277;
      state.nodes.panel!.pixelSize = 191;
      state.nodes.detail!.pixelSize = 333;
      state.nodes[DEFAULT_EDITOR_GROUP_ID]!.pixelSize = 350;
      state.nodes[created!]!.pixelSize = 450;
    });

    useLayoutStore.getState().removeTab(created!, 'g2');

    const state = useLayoutStore.getState();
    expect(state.nodes[EDITOR_AREA_ID]?.children).toEqual([DEFAULT_EDITOR_GROUP_ID]);
    expect(state.nodes[DEFAULT_EDITOR_GROUP_ID]?.pixelSize).toBe(800);
    expect(state.nodes.sidebar?.pixelSize).toBe(277);
    expect(state.nodes.panel?.pixelSize).toBe(191);
    expect(state.nodes.detail?.pixelSize).toBe(333);
  });

  it('does not expose duplicate generic editor removal or split APIs', () => {
    expect(useLayoutStore.getState()).not.toHaveProperty('removeNode');
    expect(useLayoutStore.getState()).not.toHaveProperty('splitNode');
  });
});
