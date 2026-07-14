import { beforeEach, describe, expect, it } from 'vitest';
import { useLayoutStore } from './layoutStore';
import {
  createInitialWorkbenchNodes,
  DEFAULT_EDITOR_GROUP_ID,
  EDITOR_AREA_ID,
} from './workbenchLayoutDefaults';
import { isEditorGridSash } from './editorGridLayout';

describe('toggleMaximizeEditorGroup', () => {
  beforeEach(() => {
    useLayoutStore.setState({
      nodes: createInitialWorkbenchNodes(),
      activeEditorGroupId: DEFAULT_EDITOR_GROUP_ID,
    });
  });

  it('hides sibling groups then restores ratio weights', () => {
    useLayoutStore.setState((state) => {
      state.nodes.editor_area!.children = [DEFAULT_EDITOR_GROUP_ID, 'editor_group_2'];
      state.nodes.editor_group_2 = {
        id: 'editor_group_2',
        type: 'component',
        parentId: EDITOR_AREA_ID,
        size: 0.4,
        data: { component: 'GraphEditor' },
      };
      state.nodes.default_editor!.size = 0.6;
    });

    useLayoutStore.getState().toggleMaximizeEditorGroup(DEFAULT_EDITOR_GROUP_ID);

    expect(useLayoutStore.getState().nodes.editor_group_2?.data?.groupMaximizedHidden).toBe(true);
    expect(useLayoutStore.getState().nodes[EDITOR_AREA_ID]?.data?.maximizedGroupId).toBe(DEFAULT_EDITOR_GROUP_ID);
    expect(useLayoutStore.getState().nodes[EDITOR_AREA_ID]?.data?.restoredGridWeights).toMatchObject({
      [DEFAULT_EDITOR_GROUP_ID]: 0.6,
      editor_group_2: 0.4,
    });

    useLayoutStore.getState().toggleMaximizeEditorGroup(DEFAULT_EDITOR_GROUP_ID);

    expect(useLayoutStore.getState().nodes.editor_group_2?.data?.groupMaximizedHidden).toBe(false);
    expect(useLayoutStore.getState().nodes.editor_group_2?.pixelSize).toBeUndefined();
    expect(useLayoutStore.getState().nodes.default_editor?.pixelSize).toBeUndefined();
    expect(useLayoutStore.getState().nodes.editor_group_2?.size).toBeCloseTo(0.4);
    expect(useLayoutStore.getState().nodes.default_editor?.size).toBeCloseTo(0.6);
  });
});

describe('isEditorGridSash', () => {
  it('distinguishes editor grid sashes from workbench panel sashes', () => {
    const nodes = createInitialWorkbenchNodes();
    expect(isEditorGridSash('default_editor', 'editor_group_2', {
      ...nodes,
      editor_group_2: {
        id: 'editor_group_2',
        type: 'component',
        parentId: EDITOR_AREA_ID,
        data: { component: 'GraphEditor' },
      },
    })).toBe(true);
    expect(isEditorGridSash(EDITOR_AREA_ID, 'panel', nodes)).toBe(false);
  });
});

describe('resetEditorGridSplitEqual', () => {
  beforeEach(() => {
    useLayoutStore.setState({
      nodes: {
        ...createInitialWorkbenchNodes(),
        a: { id: 'a', type: 'col', parentId: EDITOR_AREA_ID, children: [] },
        b: { id: 'b', type: 'col', parentId: EDITOR_AREA_ID, children: [] },
      },
    });
  });

  it('splits total size evenly between siblings as ratio weights', () => {
    useLayoutStore.getState().resetEditorGridSplitEqual('a', 'b', 300, 500);
    expect(useLayoutStore.getState().nodes.a?.size).toBeCloseTo(0.5);
    expect(useLayoutStore.getState().nodes.b?.size).toBeCloseTo(0.5);
    expect(useLayoutStore.getState().nodes.a?.pixelSize).toBeUndefined();
    expect(useLayoutStore.getState().nodes.b?.pixelSize).toBeUndefined();
  });
});
