// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { createInitialWorkbenchNodes, DEFAULT_EDITOR_GROUP_ID, EDITOR_AREA_ID } from './workbenchLayoutDefaults';
import {
  loadWorkbenchLayoutMemento,
  setWorkbenchLayoutWindowScope,
} from './workbenchLayoutMemento';
import { mergeWorkbenchLayoutMemento } from './workbenchLayoutPersistence';
import {
  collapseEditorGroupsForProjectSwitch,
  persistEditorGridDebounced,
  persistEditorTabsDebounced,
  persistWorkbenchLayoutNow,
  persistWorkbenchLayoutDebounced,
  hydrateWorkbenchChrome,
  reclampWorkbenchPanelSize,
  resetWorkbenchLayout,
  subscribeWorkbenchViewportResize,
} from './workbenchLayoutService';
import { useLayoutStore } from './layoutStore';
import { useEditorTabStore } from './editorTabStore';
import { resetEditorTabStore, seedEditorGroupTabs } from './editorTabTestUtils';
import { snapshotEditorGridMemento } from './editorGridMemento';
import { enterZenMode } from './workbenchZenMode';
import { restoreAdjacentPanelVisibility } from '@/views/EditorView/Renderer/sashResizeLogic';

describe('workbenchLayoutPersistence decoupling', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    localStorage.clear();
    setWorkbenchLayoutWindowScope('main');
    resetEditorTabStore();
    useLayoutStore.setState({
      rootId: 'root',
      nodes: createInitialWorkbenchNodes(),
      activeEditorGroupId: DEFAULT_EDITOR_GROUP_ID,
      zenMode: false,
    });
  });

  it('persistEditorGridDebounced merges only editorGrid and preserves chrome parts', () => {
    mergeWorkbenchLayoutMemento({
      parts: { sidebar: { pixelSize: 300, visible: true } },
      editorGrid: null,
    });

    useLayoutStore.setState((state) => {
      state.nodes.editor_area!.children = [DEFAULT_EDITOR_GROUP_ID, 'editor_group_2'];
      state.nodes.editor_group_2 = {
        id: 'editor_group_2',
        type: 'component',
        parentId: EDITOR_AREA_ID,
        data: { component: 'GraphEditor' },
      };
    });

    persistEditorGridDebounced();
    vi.advanceTimersByTime(300);

    const memento = loadWorkbenchLayoutMemento();
    expect(memento?.parts?.sidebar?.pixelSize).toBe(300);
    expect(memento?.editorGrid?.nodes?.some((n) => n.id === 'editor_group_2')).toBe(true);
  });

  it('persists editor tab order and activation independently from editorGrid', () => {
    seedEditorGroupTabs(DEFAULT_EDITOR_GROUP_ID, [
      { id: 'events/a', type: 'event', component: 'GraphEditor' },
      { id: 'events/b', type: 'event', component: 'GraphEditor' },
    ]);
    useEditorTabStore.getState().setActiveTab(DEFAULT_EDITOR_GROUP_ID, 'events/a');
    persistEditorTabsDebounced();
    vi.advanceTimersByTime(300);

    const memento = loadWorkbenchLayoutMemento();
    expect(memento?.editorTabs?.placements[DEFAULT_EDITOR_GROUP_ID]).toMatchObject({
      tabIds: ['events/a', 'events/b'],
      activeTabId: 'events/a',
    });
    expect(memento?.editorGrid).toBeNull();
  });

  it('persistWorkbenchLayoutDebounced merges only parts and preserves editorGrid', () => {
    const grid = snapshotEditorGridMemento(useLayoutStore.getState().nodes, DEFAULT_EDITOR_GROUP_ID);
    mergeWorkbenchLayoutMemento({ parts: { sidebar: { pixelSize: 240 } }, editorGrid: grid });

    useLayoutStore.getState().resizeNode('sidebar', 320);
    persistWorkbenchLayoutDebounced();
    vi.advanceTimersByTime(300);

    const memento = loadWorkbenchLayoutMemento();
    expect(memento?.parts?.sidebar?.pixelSize).toBe(320);
    expect(memento?.editorGrid?.activeEditorGroupId).toBe(DEFAULT_EDITOR_GROUP_ID);
  });

  it('persists chrome and editorGrid when both are scheduled within one debounce window', () => {
    mergeWorkbenchLayoutMemento({
      parts: { sidebar: { pixelSize: 240 } },
      editorGrid: snapshotEditorGridMemento(useLayoutStore.getState().nodes, DEFAULT_EDITOR_GROUP_ID),
    });

    useLayoutStore.getState().resizeNode('sidebar', 320);
    persistWorkbenchLayoutDebounced();
    useLayoutStore.setState((state) => {
      state.nodes[EDITOR_AREA_ID]!.children = [DEFAULT_EDITOR_GROUP_ID, 'editor_group_2'];
      state.nodes.editor_group_2 = {
        id: 'editor_group_2',
        type: 'component',
        parentId: EDITOR_AREA_ID,
        data: { component: 'GraphEditor' },
      };
      state.activeEditorGroupId = 'editor_group_2';
    });
    persistEditorGridDebounced();
    vi.advanceTimersByTime(300);

    const memento = loadWorkbenchLayoutMemento();
    expect(memento?.parts?.sidebar?.pixelSize).toBe(320);
    expect(memento?.editorGrid?.activeEditorGroupId).toBe('editor_group_2');
    expect(memento?.editorGrid?.nodes.some((node) => node.id === 'editor_group_2')).toBe(true);
  });

  it('does not persist Zen-hidden chrome from a queued chrome write', () => {
    const grid = snapshotEditorGridMemento(
      useLayoutStore.getState().nodes,
      DEFAULT_EDITOR_GROUP_ID,
    );
    mergeWorkbenchLayoutMemento({
      parts: { sidebar: { pixelSize: 240, visible: true } },
      editorGrid: grid,
    });

    useLayoutStore.getState().resizeNode('sidebar', 320);
    persistWorkbenchLayoutDebounced();
    enterZenMode();
    vi.advanceTimersByTime(300);

    const memento = loadWorkbenchLayoutMemento();
    expect(memento?.parts.sidebar).toMatchObject({ pixelSize: 320, visible: true });
    expect(memento?.editorGrid).toEqual(grid);
  });

  it('does not persist chrome visibility or size changes from sash drag restore/commit during Zen', () => {
    mergeWorkbenchLayoutMemento({
      parts: {
        sidebar: { pixelSize: 260, visible: true },
        panel: { pixelSize: 200, visible: false },
      },
      editorGrid: snapshotEditorGridMemento(
        useLayoutStore.getState().nodes,
        DEFAULT_EDITOR_GROUP_ID,
      ),
    });

    useLayoutStore.setState((state) => {
      state.nodes.panel!.data = { ...state.nodes.panel!.data, visible: false };
    });

    enterZenMode();

    restoreAdjacentPanelVisibility(EDITOR_AREA_ID, 'panel');
    useLayoutStore.getState().resizeNode('panel', 280);
    persistWorkbenchLayoutDebounced();
    vi.advanceTimersByTime(300);

    const memento = loadWorkbenchLayoutMemento();
    expect(memento?.parts.panel).toMatchObject({ pixelSize: 200, visible: false });
    expect(memento?.parts.sidebar).toMatchObject({ pixelSize: 260, visible: true });
    expect(memento?.editorGrid?.activeEditorGroupId).toBe(DEFAULT_EDITOR_GROUP_ID);
  });

  it('still persists editor grid changes scheduled during Zen', () => {
    const grid = snapshotEditorGridMemento(
      useLayoutStore.getState().nodes,
      DEFAULT_EDITOR_GROUP_ID,
    );
    mergeWorkbenchLayoutMemento({
      parts: { sidebar: { pixelSize: 260, visible: true } },
      editorGrid: grid,
    });

    enterZenMode();
    useLayoutStore.setState((state) => {
      state.nodes[EDITOR_AREA_ID]!.children = [DEFAULT_EDITOR_GROUP_ID, 'editor_group_2'];
      state.nodes.editor_group_2 = {
        id: 'editor_group_2',
        type: 'component',
        parentId: EDITOR_AREA_ID,
        data: { component: 'GraphEditor' },
      };
      state.activeEditorGroupId = 'editor_group_2';
    });
    persistEditorGridDebounced();
    vi.advanceTimersByTime(300);

    const memento = loadWorkbenchLayoutMemento();
    expect(memento?.parts?.sidebar).toMatchObject({ pixelSize: 260, visible: true });
    expect(memento?.editorGrid?.activeEditorGroupId).toBe('editor_group_2');
    expect(memento?.editorGrid?.nodes.some((node) => node.id === 'editor_group_2')).toBe(true);
  });

  it('does not persist Zen-hidden chrome from viewport reclamp scheduled while Zen is active', () => {
    Object.defineProperty(window, 'innerWidth', { configurable: true, value: 900 });
    Object.defineProperty(window, 'innerHeight', { configurable: true, value: 700 });

    mergeWorkbenchLayoutMemento({
      parts: {
        sidebar: { pixelSize: 260, visible: true },
        panel: { pixelSize: 220, visible: true },
      },
    });

    useLayoutStore.setState((state) => {
      state.nodes.center!.type = 'row';
      state.nodes.center!.children = [EDITOR_AREA_ID, 'panel'];
      state.nodes.panel!.pixelSize = 900;
    });

    enterZenMode();
    reclampWorkbenchPanelSize();
    vi.advanceTimersByTime(300);

    const memento = loadWorkbenchLayoutMemento();
    expect(memento?.parts.panel).toMatchObject({ pixelSize: 220, visible: true });
    expect(memento?.parts.sidebar).toMatchObject({ pixelSize: 260, visible: true });
  });

  it('Reset Layout preserves editor groups, tabs, and the active tab', () => {
    useLayoutStore.setState((state) => {
      state.nodes.sidebar!.pixelSize = 480;
      state.nodes.sidebar!.data = {
        ...state.nodes.sidebar!.data,
        visible: false,
        currentTab: 'charts',
      };
      state.nodes.center!.type = 'row';
      state.nodes.center!.children = ['panel', EDITOR_AREA_ID];
      state.nodes.panel!.pixelSize = 540;
      state.nodes.panel!.data = {
        ...state.nodes.panel!.data,
        visible: false,
        maximized: true,
        restoredPixelSize: 310,
      };
      state.nodes.detail!.pixelSize = 450;
      state.nodes.detail!.data = {
        ...state.nodes.detail!.data,
        visible: false,
        userHidden: true,
      };
      state.nodes[EDITOR_AREA_ID]!.children = [DEFAULT_EDITOR_GROUP_ID, 'editor_group_2'];
      state.nodes.editor_group_2 = {
        id: 'editor_group_2',
        type: 'component',
        parentId: EDITOR_AREA_ID,
        data: { component: 'GraphEditor' },
      };
      state.activeEditorGroupId = 'editor_group_2';
    });
    seedEditorGroupTabs(
      DEFAULT_EDITOR_GROUP_ID,
      [{ id: 'events/one', component: 'GraphEditor', type: 'event' }],
      'events/one',
      ['node-one'],
    );
    seedEditorGroupTabs(
      'editor_group_2',
      [{ id: 'events/two', component: 'GraphEditor', type: 'event' }],
      'events/two',
      ['node-two'],
    );

    resetWorkbenchLayout();

    const state = useLayoutStore.getState();
    const tabState = useEditorTabStore.getState();
    expect(state.nodes[EDITOR_AREA_ID]?.children).toEqual([DEFAULT_EDITOR_GROUP_ID, 'editor_group_2']);
    expect(tabState.getPlacement(DEFAULT_EDITOR_GROUP_ID).tabIds).toEqual(['events/one']);
    expect(tabState.getPlacement('editor_group_2').tabIds).toEqual(['events/two']);
    expect(tabState.getPlacement('editor_group_2').activeTabId).toBe('events/two');
    expect(tabState.getPlacement(DEFAULT_EDITOR_GROUP_ID).selectedNodeIds).toEqual(['node-one']);
    expect(tabState.getPlacement('editor_group_2').selectedNodeIds).toEqual(['node-two']);
    expect(state.activeEditorGroupId).toBe('editor_group_2');
    expect(state.nodes.sidebar?.pixelSize).toBe(260);
    expect(state.nodes.sidebar?.data?.visible).toBe(true);
    expect(state.nodes.sidebar?.data?.currentTab).toBe('graphs');
    expect(state.nodes.center?.type).toBe('col');
    expect(state.nodes.center?.children).toEqual([EDITOR_AREA_ID, 'panel']);
    expect(state.nodes.panel?.pixelSize).toBe(200);
    expect(state.nodes.panel?.data?.visible).toBe(true);
    expect(state.nodes.panel?.data?.maximized).not.toBe(true);
    expect(state.nodes.panel?.data?.restoredPixelSize).toBeUndefined();
    expect(state.nodes.detail?.pixelSize).toBe(300);
    expect(state.nodes.detail?.data?.visible).toBe(true);
    expect(state.nodes.detail?.data?.userHidden).toBe(false);

    const persisted = loadWorkbenchLayoutMemento();
    expect(persisted?.editorGrid?.activeEditorGroupId).toBe('editor_group_2');
    expect(persisted?.editorGrid?.nodes.some((node) => node.id === 'editor_group_2')).toBe(true);
    expect(persisted?.parts.sidebar?.pixelSize).toBe(260);
  });

  it('scopes the workbench layout memento to each editor window', () => {
    setWorkbenchLayoutWindowScope('main');
    useLayoutStore.getState().resizeNode('sidebar', 260);
    persistWorkbenchLayoutNow();

    setWorkbenchLayoutWindowScope('window-2');
    useLayoutStore.getState().resizeNode('sidebar', 420);
    persistWorkbenchLayoutNow();

    useLayoutStore.getState().resizeNode('sidebar', 240);
    setWorkbenchLayoutWindowScope('main');
    hydrateWorkbenchChrome();
    expect(useLayoutStore.getState().nodes.sidebar?.pixelSize).toBe(260);

    useLayoutStore.getState().resizeNode('sidebar', 240);
    setWorkbenchLayoutWindowScope('window-2');
    hydrateWorkbenchChrome();
    expect(useLayoutStore.getState().nodes.sidebar?.pixelSize).toBe(420);
  });

  it('reclamps a persisted side panel against viewport width', () => {
    Object.defineProperty(window, 'innerWidth', { configurable: true, value: 1_000 });
    Object.defineProperty(window, 'innerHeight', { configurable: true, value: 700 });
    useLayoutStore.setState((state) => {
      state.nodes.center!.type = 'row';
      state.nodes.center!.children = ['panel', EDITOR_AREA_ID];
      state.nodes.panel!.pixelSize = 1_100;
    });

    reclampWorkbenchPanelSize();

    expect(useLayoutStore.getState().nodes.panel?.pixelSize).toBe(800);
  });

  it('hydrates a side panel using the horizontal viewport clamp', () => {
    Object.defineProperty(window, 'innerWidth', { configurable: true, value: 1_200 });
    Object.defineProperty(window, 'innerHeight', { configurable: true, value: 700 });
    mergeWorkbenchLayoutMemento({
      parts: { panel: { pixelSize: 1_100, visible: true } },
    });
    useLayoutStore.setState((state) => {
      state.nodes.center!.type = 'row';
      state.nodes.center!.children = ['panel', EDITOR_AREA_ID];
    });

    hydrateWorkbenchChrome();

    expect(useLayoutStore.getState().nodes.panel?.pixelSize).toBe(960);
  });

  it('registers a disposable viewport resize reclamp listener', () => {
    Object.defineProperty(window, 'innerWidth', { configurable: true, value: 1_000 });
    Object.defineProperty(window, 'innerHeight', { configurable: true, value: 700 });
    useLayoutStore.setState((state) => {
      state.nodes.center!.type = 'row';
      state.nodes.center!.children = [EDITOR_AREA_ID, 'panel'];
      state.nodes.panel!.pixelSize = 1_100;
    });
    const dispose = subscribeWorkbenchViewportResize(50);

    window.dispatchEvent(new Event('resize'));
    vi.advanceTimersByTime(50);
    expect(useLayoutStore.getState().nodes.panel?.pixelSize).toBe(800);

    useLayoutStore.setState((state) => {
      state.nodes.panel!.pixelSize = 1_100;
    });
    dispose();
    window.dispatchEvent(new Event('resize'));
    vi.advanceTimersByTime(50);
    expect(useLayoutStore.getState().nodes.panel?.pixelSize).toBe(1_100);
  });

  it('collapseEditorGroupsForProjectSwitch persists single-group grid memento', () => {
    useLayoutStore.setState((state) => {
      state.nodes.editor_area!.children = [DEFAULT_EDITOR_GROUP_ID, 'editor_group_2'];
      state.nodes.editor_group_2 = {
        id: 'editor_group_2',
        type: 'component',
        parentId: EDITOR_AREA_ID,
        data: { component: 'GraphEditor' },
      };
    });
    mergeWorkbenchLayoutMemento({
      parts: { sidebar: { pixelSize: 260 } },
      editorGrid: snapshotEditorGridMemento(useLayoutStore.getState().nodes, DEFAULT_EDITOR_GROUP_ID),
    });

    collapseEditorGroupsForProjectSwitch();

    const memento = loadWorkbenchLayoutMemento();
    expect(memento?.parts?.sidebar?.pixelSize).toBe(260);
    expect(memento?.editorGrid?.nodes?.some((n) => n.id === 'editor_group_2')).toBe(false);
    expect(memento?.editorGrid?.nodes?.some((n) => n.id === DEFAULT_EDITOR_GROUP_ID)).toBe(true);
  });
});
