// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { createInitialWorkbenchNodes, DEFAULT_EDITOR_GROUP_ID, EDITOR_AREA_ID } from './workbenchLayoutDefaults';
import { loadWorkbenchLayoutMemento } from './workbenchLayoutMemento';
import { mergeWorkbenchLayoutMemento } from './workbenchLayoutPersistence';
import {
  collapseEditorGroupsForProjectSwitch,
  persistEditorGridDebounced,
  persistWorkbenchLayoutDebounced,
} from './workbenchLayoutService';
import { useLayoutStore } from './layoutStore';
import { snapshotEditorGridMemento } from './editorGridMemento';

describe('workbenchLayoutPersistence decoupling', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    localStorage.clear();
    useLayoutStore.setState({
      rootId: 'root',
      nodes: createInitialWorkbenchNodes(),
      activeEditorGroupId: DEFAULT_EDITOR_GROUP_ID,
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
        data: { component: 'GraphEditor', tabs: [] },
      };
    });

    persistEditorGridDebounced();
    vi.advanceTimersByTime(300);

    const memento = loadWorkbenchLayoutMemento();
    expect(memento?.parts?.sidebar?.pixelSize).toBe(300);
    expect(memento?.editorGrid?.nodes?.some((n) => n.id === 'editor_group_2')).toBe(true);
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

  it('collapseEditorGroupsForProjectSwitch persists single-group grid memento', () => {
    useLayoutStore.setState((state) => {
      state.nodes.editor_area!.children = [DEFAULT_EDITOR_GROUP_ID, 'editor_group_2'];
      state.nodes.editor_group_2 = {
        id: 'editor_group_2',
        type: 'component',
        parentId: EDITOR_AREA_ID,
        data: { component: 'GraphEditor', tabs: [] },
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
