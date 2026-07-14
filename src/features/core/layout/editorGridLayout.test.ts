import { describe, expect, it } from 'vitest';
import { createInitialWorkbenchNodes, DEFAULT_EDITOR_GROUP_ID, EDITOR_AREA_ID } from './workbenchLayoutDefaults';
import {
  applyEqualGridSplit,
  removeEditorGroupFromTree,
  resolveEditorGroupMinSize,
  setEditorGroupMaximizedHidden,
  splitEditorGroupInTree,
  writeEditorAreaMaximizeState,
} from './editorGridLayout';
import { resetEditorTabStore, seedEditorGroupTabs } from './editorTabTestUtils';
import { useEditorTabStore } from './editorTabStore';

describe('editorGridLayout tree ops', () => {
  it('splitEditorGroupInTree forks a sibling group to the right with equal halves', () => {
    const nodes = createInitialWorkbenchNodes();
    const newId = splitEditorGroupInTree(nodes, DEFAULT_EDITOR_GROUP_ID, 'right', {
      component: 'GraphEditor',
    });
    expect(newId).toBeTruthy();
    expect(nodes[EDITOR_AREA_ID]?.children?.length).toBe(2);
    expect(nodes[DEFAULT_EDITOR_GROUP_ID]?.pixelSize).toBeUndefined();
    expect(nodes[newId!]?.pixelSize).toBeUndefined();
    expect(nodes[DEFAULT_EDITOR_GROUP_ID]?.size).toBeCloseTo(0.5);
    expect(nodes[newId!]?.size).toBeCloseTo(0.5);
  });

  it('splitEditorGroupInTree distributes evenly when adding a third equal sibling', () => {
    const nodes = createInitialWorkbenchNodes();
    const right = splitEditorGroupInTree(nodes, DEFAULT_EDITOR_GROUP_ID, 'right', {
      component: 'GraphEditor',
    });
    expect(right).toBeTruthy();

    const farRight = splitEditorGroupInTree(nodes, right!, 'right', {
      component: 'GraphEditor',
    });
    expect(farRight).toBeTruthy();

    expect(nodes[DEFAULT_EDITOR_GROUP_ID]?.size).toBeCloseTo(1 / 3);
    expect(nodes[right!]?.size).toBeCloseTo(1 / 3);
    expect(nodes[farRight!]?.size).toBeCloseTo(1 / 3);
    expect(nodes[DEFAULT_EDITOR_GROUP_ID]?.pixelSize).toBeUndefined();
    expect(nodes[right!]?.pixelSize).toBeUndefined();
    expect(nodes[farRight!]?.pixelSize).toBeUndefined();
  });

  it('halves the target group when inserting a third same-axis sibling', () => {
    const nodes = createInitialWorkbenchNodes();
    const right = splitEditorGroupInTree(nodes, DEFAULT_EDITOR_GROUP_ID, 'right', {
      component: 'GraphEditor',
    });
    expect(right).toBeTruthy();

    nodes[DEFAULT_EDITOR_GROUP_ID]!.size = 0.2;
    nodes[right!]!.size = 0.8;

    const farRight = splitEditorGroupInTree(nodes, right!, 'right', {
      component: 'GraphEditor',
    });
    expect(farRight).toBeTruthy();

    expect(nodes[DEFAULT_EDITOR_GROUP_ID]?.size).toBeCloseTo(0.2);
    expect(nodes[right!]?.size).toBeCloseTo(0.4);
    expect(nodes[farRight!]?.size).toBeCloseTo(0.4);
    expect(nodes[DEFAULT_EDITOR_GROUP_ID]?.pixelSize).toBeUndefined();
    expect(nodes[right!]?.pixelSize).toBeUndefined();
    expect(nodes[farRight!]?.pixelSize).toBeUndefined();
  });

  it('does not collapse sash-sized pairs into equal thirds on third insert', () => {
    const nodes = createInitialWorkbenchNodes();
    const right = splitEditorGroupInTree(nodes, DEFAULT_EDITOR_GROUP_ID, 'right', {
      component: 'GraphEditor',
    });
    expect(right).toBeTruthy();

    nodes[DEFAULT_EDITOR_GROUP_ID]!.pixelSize = 200;
    nodes[right!]!.pixelSize = 800;

    const farRight = splitEditorGroupInTree(nodes, right!, 'right', {
      component: 'GraphEditor',
    });
    expect(farRight).toBeTruthy();

    expect(nodes[DEFAULT_EDITOR_GROUP_ID]?.size).toBeCloseTo(0.2);
    expect(nodes[right!]?.size).toBeCloseTo(0.4);
    expect(nodes[farRight!]?.size).toBeCloseTo(0.4);
    expect(nodes[farRight!]?.size).toBeGreaterThan(0.1);
  });

  it('applyEqualGridSplit divides pair evenly', () => {
    const nodes = createInitialWorkbenchNodes();
    nodes.a = { id: 'a', type: 'component', parentId: EDITOR_AREA_ID, pixelSize: 300 };
    nodes.b = { id: 'b', type: 'component', parentId: EDITOR_AREA_ID, pixelSize: 500 };
    applyEqualGridSplit(nodes, 'a', 'b', 300, 500);
    expect(nodes.a?.pixelSize).toBe(400);
    expect(nodes.b?.pixelSize).toBe(400);
  });

  it('uses axis-aware editor group minimums while honoring node overrides', () => {
    const node = createInitialWorkbenchNodes()[DEFAULT_EDITOR_GROUP_ID];
    expect(resolveEditorGroupMinSize(node, 'row')).toBe(200);
    expect(resolveEditorGroupMinSize(node, 'col')).toBe(120);
    expect(resolveEditorGroupMinSize({ ...node!, minSize: 260 }, 'row')).toBe(260);
  });

  it('removeEditorGroupFromTree reflows the survivor to fill the editor area', () => {
    resetEditorTabStore();
    const nodes = createInitialWorkbenchNodes();
    nodes.sidebar!.pixelSize = 271;
    nodes.panel!.pixelSize = 183;
    nodes.detail!.pixelSize = 319;
    nodes[DEFAULT_EDITOR_GROUP_ID]!.pixelSize = 320;
    const created = splitEditorGroupInTree(nodes, DEFAULT_EDITOR_GROUP_ID, 'right', {
      component: 'GraphEditor',
    });
    expect(created).toBeTruthy();
    nodes[created!].pixelSize = 480;
    seedEditorGroupTabs(created!, [{ id: 'events/bar', component: 'GraphEditor', type: 'event' }]);
    useEditorTabStore.getState().removeTab(created!, 'events/bar');

    const { removed, nextActiveGroupId } = removeEditorGroupFromTree(nodes, created!);
    expect(removed).toBe(true);
    expect(nodes[created!]).toBeUndefined();
    expect(nextActiveGroupId).toBe(DEFAULT_EDITOR_GROUP_ID);
    expect(nodes[EDITOR_AREA_ID]?.children).toEqual([DEFAULT_EDITOR_GROUP_ID]);
    expect(nodes[DEFAULT_EDITOR_GROUP_ID]?.parentId).toBe(EDITOR_AREA_ID);
    expect(nodes[DEFAULT_EDITOR_GROUP_ID]?.pixelSize).toBeUndefined();
    expect(nodes[DEFAULT_EDITOR_GROUP_ID]?.size).toBe(1);
    expect(nodes.sidebar?.pixelSize).toBe(271);
    expect(nodes.panel?.pixelSize).toBe(183);
    expect(nodes.detail?.pixelSize).toBe(319);
  });

  it('preserves parent-axis width without reusing it as perpendicular child height', () => {
    const nodes = createInitialWorkbenchNodes();
    nodes[DEFAULT_EDITOR_GROUP_ID]!.pixelSize = 640;

    const created = splitEditorGroupInTree(nodes, DEFAULT_EDITOR_GROUP_ID, 'bottom', {
      component: 'GraphEditor',
    });

    expect(created).toBeTruthy();
    const branchId = nodes[DEFAULT_EDITOR_GROUP_ID]?.parentId;
    expect(branchId).toBeTruthy();
    expect(nodes[branchId!]?.pixelSize).toBeUndefined();
    expect(nodes[branchId!]?.size).toBe(1);
    expect(nodes[DEFAULT_EDITOR_GROUP_ID]?.pixelSize).toBeUndefined();
    expect(nodes[created!]?.pixelSize).toBeUndefined();
    expect(nodes[DEFAULT_EDITOR_GROUP_ID]?.size).toBeCloseTo(0.5);
    expect(nodes[created!]?.size).toBeCloseTo(0.5);
  });

  it('clears stale maximize/hidden flags when the maximized group is removed', () => {
    resetEditorTabStore();
    const nodes = createInitialWorkbenchNodes();
    nodes[DEFAULT_EDITOR_GROUP_ID]!.pixelSize = 400;
    const created = splitEditorGroupInTree(nodes, DEFAULT_EDITOR_GROUP_ID, 'right', {
      component: 'GraphEditor',
    });
    expect(created).toBeTruthy();
    nodes[created!].pixelSize = 400;
    seedEditorGroupTabs(created!, [{ id: 'events/bar', component: 'GraphEditor', type: 'event' }]);

    setEditorGroupMaximizedHidden(nodes, created!, false);
    setEditorGroupMaximizedHidden(nodes, DEFAULT_EDITOR_GROUP_ID, true);
    writeEditorAreaMaximizeState(nodes, created!, {
      [DEFAULT_EDITOR_GROUP_ID]: 400,
      [created!]: 400,
    });

    useEditorTabStore.getState().removeTab(created!, 'events/bar');
    const { removed } = removeEditorGroupFromTree(nodes, created!);
    expect(removed).toBe(true);
    expect(nodes[DEFAULT_EDITOR_GROUP_ID]?.data?.groupMaximizedHidden).toBe(false);
    expect(nodes[EDITOR_AREA_ID]?.data?.maximizedGroupId).toBeUndefined();
    expect(nodes[EDITOR_AREA_ID]?.children).toEqual([DEFAULT_EDITOR_GROUP_ID]);
  });

  it('reflows a vertical split collapse without cross-axis pixel locks', () => {
    resetEditorTabStore();
    const nodes = createInitialWorkbenchNodes();
    nodes[DEFAULT_EDITOR_GROUP_ID]!.pixelSize = 300;
    const created = splitEditorGroupInTree(nodes, DEFAULT_EDITOR_GROUP_ID, 'bottom', {
      component: 'GraphEditor',
    });
    expect(created).toBeTruthy();
    nodes[DEFAULT_EDITOR_GROUP_ID]!.pixelSize = 300;
    nodes[created!].pixelSize = 500;
    seedEditorGroupTabs(created!, [{ id: 'events/bar', component: 'GraphEditor', type: 'event' }]);
    useEditorTabStore.getState().removeTab(created!, 'events/bar');

    removeEditorGroupFromTree(nodes, created!);

    expect(nodes[EDITOR_AREA_ID]?.children).toEqual([DEFAULT_EDITOR_GROUP_ID]);
    expect(nodes[DEFAULT_EDITOR_GROUP_ID]?.pixelSize).toBeUndefined();
    expect(nodes[DEFAULT_EDITOR_GROUP_ID]?.size).toBe(1);
  });

  it('renormalizes remaining sash-sized groups after closing one of three', () => {
    resetEditorTabStore();
    const nodes = createInitialWorkbenchNodes();
    const middle = splitEditorGroupInTree(nodes, DEFAULT_EDITOR_GROUP_ID, 'right', {
      component: 'GraphEditor',
    });
    expect(middle).toBeTruthy();
    const farRight = splitEditorGroupInTree(nodes, middle!, 'right', {
      component: 'GraphEditor',
    });
    expect(farRight).toBeTruthy();
    seedEditorGroupTabs(middle!, [{ id: 'events/middle', component: 'GraphEditor', type: 'event' }]);
    seedEditorGroupTabs(farRight!, [{ id: 'events/far', component: 'GraphEditor', type: 'event' }]);

    nodes[DEFAULT_EDITOR_GROUP_ID]!.pixelSize = 200;
    nodes[middle!]!.pixelSize = 300;
    nodes[farRight!]!.pixelSize = 500;

    useEditorTabStore.getState().removeTab(middle!, 'events/middle');
    removeEditorGroupFromTree(nodes, middle!);

    expect(nodes[middle!]).toBeUndefined();
    expect(nodes[EDITOR_AREA_ID]?.children).toEqual([DEFAULT_EDITOR_GROUP_ID, farRight!]);
    expect(nodes[DEFAULT_EDITOR_GROUP_ID]?.pixelSize).toBeUndefined();
    expect(nodes[farRight!]?.pixelSize).toBeUndefined();
    expect(nodes[DEFAULT_EDITOR_GROUP_ID]?.size).toBeCloseTo(0.5);
    expect(nodes[farRight!]?.size).toBeCloseTo(0.5);
  });
});
