import { beforeEach, describe, expect, it } from 'vitest';
import { useLayoutStore } from './layoutStore';
import {
  createInitialWorkbenchNodes,
  DEFAULT_EDITOR_GROUP_ID,
  EDITOR_AREA_ID,
} from './workbenchLayoutDefaults';

describe('collapseEditorGroups', () => {
  beforeEach(() => {
    useLayoutStore.setState({
      rootId: 'root',
      nodes: createInitialWorkbenchNodes(),
      activeEditorGroupId: DEFAULT_EDITOR_GROUP_ID,
    });
  });

  it('preserves editor_area so GraphEditor and panel order stay intact after project reset', () => {
    useLayoutStore.getState().collapseEditorGroups();

    const { nodes } = useLayoutStore.getState();
    expect(nodes[EDITOR_AREA_ID]).toBeDefined();
    expect(nodes[EDITOR_AREA_ID]?.children).toEqual([DEFAULT_EDITOR_GROUP_ID]);
    expect(nodes[DEFAULT_EDITOR_GROUP_ID]?.data?.component).toBe('GraphEditor');
    expect(nodes.center?.children).toEqual([EDITOR_AREA_ID, 'panel']);
  });

  it('removes extra split groups but keeps default_editor', () => {
    useLayoutStore.setState((state) => {
      state.nodes.editor_area!.children = [DEFAULT_EDITOR_GROUP_ID, 'editor_group_2'];
      state.nodes.editor_group_2 = {
        id: 'editor_group_2',
        type: 'component',
        parentId: EDITOR_AREA_ID,
        data: { component: 'GraphEditor', tabs: [] },
      };
    });

    useLayoutStore.getState().collapseEditorGroups();

    const { nodes } = useLayoutStore.getState();
    expect(nodes[EDITOR_AREA_ID]).toBeDefined();
    expect(nodes.editor_group_2).toBeUndefined();
    expect(nodes[EDITOR_AREA_ID]?.children).toEqual([DEFAULT_EDITOR_GROUP_ID]);
  });
});
