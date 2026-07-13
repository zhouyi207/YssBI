import { beforeEach, describe, expect, it } from 'vitest';
import { useLayoutStore } from './layoutStore';
import { useEditorTabStore } from './editorTabStore';
import {
  createInitialWorkbenchNodes,
  DEFAULT_EDITOR_GROUP_ID,
  EDITOR_AREA_ID,
} from './workbenchLayoutDefaults';

describe('editor tab placement lifecycle', () => {
  beforeEach(() => {
    useEditorTabStore.setState({ registry: {}, placements: {} });
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
      activeEditorGroupId: 'editor',
    });
    useEditorTabStore.getState().initGroupPlacement('editor', [
      { id: 'g1', component: 'GraphEditor', type: 'event' },
      { id: 'g2', component: 'GraphEditor', type: 'event' },
    ], 'g1');
    useEditorTabStore.getState().setSelectedNodeIds('editor', ['node-from-g1']);
  });

  it('clears stale selected node ids when closing the active tab selects another tab', () => {
    useLayoutStore.getState().removeTab('editor', 'g1');

    const placement = useEditorTabStore.getState().getPlacement('editor');
    expect(placement.activeTabId).toBe('g2');
    expect(placement.selectedNodeIds).toEqual([]);
  });

  it('clears stale selected node ids when activating an existing tab', () => {
    useLayoutStore.getState().addTab('editor', {
      id: 'g2',
      component: 'GraphEditor',
      type: 'event',
    });

    const placement = useEditorTabStore.getState().getPlacement('editor');
    expect(placement.activeTabId).toBe('g2');
    expect(placement.selectedNodeIds).toEqual([]);
  });

  it('keeps selected node ids when activating the already active tab', () => {
    useLayoutStore.getState().addTab('editor', {
      id: 'g1',
      component: 'GraphEditor',
      type: 'event',
    });

    const placement = useEditorTabStore.getState().getPlacement('editor');
    expect(placement.activeTabId).toBe('g1');
    expect(placement.selectedNodeIds).toEqual(['node-from-g1']);
  });
});

describe('layoutStore editor group mutations', () => {
  it('removes the last tab through the editor-grid boundary without touching chrome', () => {
    useEditorTabStore.setState({ registry: {}, placements: {} });
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
    store.removeTab(DEFAULT_EDITOR_GROUP_ID, 'g1');
    expect(useEditorTabStore.getState().getPlacement(DEFAULT_EDITOR_GROUP_ID).tabIds).toEqual([]);
    expect(store.nodes.sidebar).toBeDefined();
    expect(store.nodes.detail).toBeDefined();
  });

  it('collapseEditorGroups merges placements into default editor', () => {
    useEditorTabStore.setState({ registry: {}, placements: {} });
    useLayoutStore.setState({
      nodes: createInitialWorkbenchNodes(),
      activeEditorGroupId: DEFAULT_EDITOR_GROUP_ID,
    });
    const store = useLayoutStore.getState();
    const created = store.splitEditorGroupAtEdge(DEFAULT_EDITOR_GROUP_ID, 'right', {
      component: 'GraphEditor',
      tabs: [{ id: 'g2', component: 'GraphEditor', type: 'function' }],
      activeTabId: 'g2',
    });
    expect(created).toBeTruthy();
    store.collapseEditorGroups();
    const editorArea = store.nodes[EDITOR_AREA_ID];
    expect(editorArea.children).toEqual([DEFAULT_EDITOR_GROUP_ID]);
    const defaultPlacement = useEditorTabStore.getState().getPlacement(DEFAULT_EDITOR_GROUP_ID);
    expect(defaultPlacement.tabIds).toContain('g2');
  });
});
