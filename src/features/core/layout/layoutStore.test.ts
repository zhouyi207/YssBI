import { beforeEach, describe, expect, it } from 'vitest';
import { useLayoutStore } from './layoutStore';
import { reconcileEditorTabPlacements, useEditorTabStore } from './editorTabStore';
import {
  clearEditorGroupGraphSelection,
  getEditorGroupGraphSelection,
  updateEditorGroupSelectedConnectionIds,
  updateEditorGroupSelectedNodeIds,
} from './layoutTabQueries';
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

  it.each(['node', 'connection'] as const)(
    'clears both selections whenever an active graph changes from a %s selection',
    (selectionKind) => {
      const select = (groupId: string) => {
        if (selectionKind === 'node') {
          useEditorTabStore.getState().setSelectedNodeIds(groupId, ['node-a']);
        } else {
          useEditorTabStore.getState().setSelectedConnectionIds(groupId, ['edge-a']);
        }
      };
      const expectCleared = (groupId: string) => {
        expect(useEditorTabStore.getState().getPlacement(groupId)).toMatchObject({
          selectedNodeIds: [],
          selectedConnectionIds: [],
        });
      };

      select('editor');
      useEditorTabStore.getState().setActiveTab('editor', 'g2');
      expectCleared('editor');

      useEditorTabStore.getState().initGroupPlacement('source', [
        { id: 's1', component: 'GraphEditor', type: 'event' },
        { id: 's2', component: 'GraphEditor', type: 'event' },
      ], 's1');
      useEditorTabStore.getState().initGroupPlacement('target', [
        { id: 't1', component: 'GraphEditor', type: 'event' },
      ], 't1');
      select('source');
      select('target');
      useEditorTabStore.getState().moveTab('source', 's1', 'target');
      expectCleared('source');
      expectCleared('target');

      useEditorTabStore.getState().initGroupPlacement('close', [
        { id: 'settings', component: 'GraphEditor', type: 'setting' },
        { id: 'close-graph', component: 'GraphEditor', type: 'event' },
      ], 'close-graph');
      select('close');
      useEditorTabStore.getState().closeAllGraphTabs();
      expect(useEditorTabStore.getState().getPlacement('close').activeTabId).toBe('settings');
      expectCleared('close');

      useEditorTabStore.getState().initGroupPlacement('merge-target', [
        { id: 'm1', component: 'GraphEditor', type: 'event' },
      ], 'm1');
      useEditorTabStore.getState().initGroupPlacement('merge-source', [
        { id: 'm2', component: 'GraphEditor', type: 'event' },
      ], 'm2');
      select('merge-target');
      useEditorTabStore.getState().mergePlacementsIntoGroup('merge-target', ['merge-source']);
      expect(useEditorTabStore.getState().getPlacement('merge-target').activeTabId).toBe('m2');
      expectCleared('merge-target');

      useEditorTabStore.getState().initGroupPlacement('duplicate-target', [
        { id: 'd1', component: 'GraphEditor', type: 'event' },
      ], 'd1');
      useEditorTabStore.getState().initGroupPlacement('duplicate-source', [
        { id: 'd2', component: 'GraphEditor', type: 'event' },
      ], 'd2');
      select('duplicate-target');
      useEditorTabStore.getState().duplicateTabReference('duplicate-target', 'd2');
      expectCleared('duplicate-target');

      useEditorTabStore.getState().setActiveTab('duplicate-target', 'd1');
      select('duplicate-target');
      useEditorTabStore.getState().duplicateGroupTabs('duplicate-source', 'duplicate-target');
      expectCleared('duplicate-target');
    },
  );


  it.each(['node', 'connection'] as const)(
    'preserves %s selection when the active graph resource is renamed',
    (selectionKind) => {
      if (selectionKind === 'node') {
        useEditorTabStore.getState().setSelectedNodeIds('editor', ['node-a']);
      } else {
        useEditorTabStore.getState().setSelectedConnectionIds('editor', ['edge-a']);
      }

      useEditorTabStore.getState().renameTabId('g1', 'g1-renamed');

      expect(useEditorTabStore.getState().getPlacement('editor')).toMatchObject({
        activeTabId: 'g1-renamed',
        selectedNodeIds: selectionKind === 'node' ? ['node-a'] : [],
        selectedConnectionIds: selectionKind === 'connection' ? ['edge-a'] : [],
      });
    },
  );

  it.each(['node', 'connection'] as const)(
    'restores and round-trips legal %s selection from an empty store',
    (selectionKind) => {
      const snapshot = useEditorTabStore.getState().snapshotMemento();
      snapshot.placements.editor.selectedNodeIds = selectionKind === 'node' ? ['node-a', 'node-a'] : [];
      snapshot.placements.editor.selectedConnectionIds = selectionKind === 'connection' ? ['edge-a', 'edge-a'] : [];
      useEditorTabStore.setState({ registry: {}, placements: {} });

      useEditorTabStore.getState().applyMemento(snapshot);

      expect(useEditorTabStore.getState().getPlacement('editor')).toMatchObject({
        activeTabId: 'g1',
        selectedNodeIds: selectionKind === 'node' ? ['node-a'] : [],
        selectedConnectionIds: selectionKind === 'connection' ? ['edge-a'] : [],
      });
    },
  );

  it('normalizes legacy and mixed memento selection with connection priority', () => {
    const snapshot = useEditorTabStore.getState().snapshotMemento();
    snapshot.placements.editor.selectedNodeIds = ['node-a', 'node-a'];
    snapshot.placements.editor.selectedConnectionIds = ['edge-a', 'edge-a'];
    useEditorTabStore.setState({ registry: {}, placements: {} });

    useEditorTabStore.getState().applyMemento(snapshot);
    expect(useEditorTabStore.getState().getPlacement('editor')).toMatchObject({
      selectedNodeIds: [],
      selectedConnectionIds: ['edge-a'],
    });

    delete (snapshot.placements.editor as { selectedConnectionIds?: string[] }).selectedConnectionIds;
    useEditorTabStore.getState().applyMemento(snapshot as never);
    expect(useEditorTabStore.getState().getPlacement('editor')).toMatchObject({
      selectedNodeIds: ['node-a'],
      selectedConnectionIds: [],
    });
  });

  it.each(['node', 'connection'] as const)(
    'preserves %s selection when reconcile keeps the active graph',
    (selectionKind) => {
      if (selectionKind === 'node') {
        useEditorTabStore.getState().setSelectedNodeIds('editor', ['node-a']);
      } else {
        useEditorTabStore.getState().setSelectedConnectionIds('editor', ['edge-a']);
      }

      reconcileEditorTabPlacements(useLayoutStore.getState().nodes);

      expect(useEditorTabStore.getState().getPlacement('editor')).toMatchObject({
        activeTabId: 'g1',
        selectedNodeIds: selectionKind === 'node' ? ['node-a'] : [],
        selectedConnectionIds: selectionKind === 'connection' ? ['edge-a'] : [],
      });
    },
  );

  it.each(['node', 'connection'] as const)(
    'preserves %s selection when tab reorder keeps the active graph',
    (selectionKind) => {
      if (selectionKind === 'node') {
        useEditorTabStore.getState().setSelectedNodeIds('editor', ['node-a']);
      } else {
        useEditorTabStore.getState().setSelectedConnectionIds('editor', ['edge-a']);
      }

      useEditorTabStore.getState().moveTab('editor', 'g2', 'editor', 0);

      expect(useEditorTabStore.getState().getPlacement('editor')).toMatchObject({
        activeTabId: 'g1',
        selectedNodeIds: selectionKind === 'node' ? ['node-a'] : [],
        selectedConnectionIds: selectionKind === 'connection' ? ['edge-a'] : [],
      });
    },
  );
});

describe('editor group graph selection', () => {
  beforeEach(() => {
    useEditorTabStore.setState({ registry: {}, placements: {} });
    useEditorTabStore.getState().ensureGroupPlacement('group-a');
    useLayoutStore.setState({ activeEditorGroupId: 'group-a' });
  });

  it('keeps node and connection selection mutually exclusive and deduplicated', () => {
    updateEditorGroupSelectedNodeIds(['node-a', 'node-a'], 'group-a');
    expect(getEditorGroupGraphSelection('group-a')).toEqual({
      nodeIds: new Set(['node-a']),
      connectionIds: new Set(),
    });

    updateEditorGroupSelectedConnectionIds(['edge-a', 'edge-a'], 'group-a');
    expect(getEditorGroupGraphSelection('group-a')).toEqual({
      nodeIds: new Set(),
      connectionIds: new Set(['edge-a']),
    });

    updateEditorGroupSelectedNodeIds((current) => [...current, 'node-b'], 'group-a');
    expect(getEditorGroupGraphSelection('group-a')).toEqual({
      nodeIds: new Set(['node-b']),
      connectionIds: new Set(),
    });
  });

  it('supports deterministic connection toggles and clears both kinds', () => {
    updateEditorGroupSelectedConnectionIds(['edge-a', 'edge-b'], 'group-a');
    updateEditorGroupSelectedConnectionIds(
      (current) => current.includes('edge-a')
        ? current.filter((id) => id !== 'edge-a')
        : [...current, 'edge-a'],
      'group-a',
    );
    expect(getEditorGroupGraphSelection('group-a').connectionIds).toEqual(new Set(['edge-b']));

    clearEditorGroupGraphSelection('group-a');
    expect(getEditorGroupGraphSelection('group-a')).toEqual({
      nodeIds: new Set(),
      connectionIds: new Set(),
    });
  });

  it('round-trips one selection kind and normalizes legacy mementos', () => {
    updateEditorGroupSelectedConnectionIds(['edge-a'], 'group-a');
    const snapshot = useEditorTabStore.getState().snapshotMemento();
    expect(snapshot.placements['group-a'].selectedConnectionIds).toEqual(['edge-a']);

    useEditorTabStore.getState().applyMemento(snapshot);
    expect(getEditorGroupGraphSelection('group-a').connectionIds).toEqual(new Set(['edge-a']));

    const legacy = structuredClone(snapshot) as unknown as {
      registry: typeof snapshot.registry;
      placements: Record<string, Omit<(typeof snapshot.placements)[string], 'selectedConnectionIds'>>;
    };
    delete (legacy.placements['group-a'] as { selectedConnectionIds?: string[] }).selectedConnectionIds;
    useEditorTabStore.getState().applyMemento(legacy as never);
    expect(getEditorGroupGraphSelection('group-a').connectionIds).toEqual(new Set());
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
