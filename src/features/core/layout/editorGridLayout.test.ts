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
  it('splitEditorGroupInTree forks a sibling group to the right', () => {
    const nodes = createInitialWorkbenchNodes();
    const newId = splitEditorGroupInTree(nodes, DEFAULT_EDITOR_GROUP_ID, 'right', {
      component: 'GraphEditor',
    });
    expect(newId).toBeTruthy();
    expect(nodes[EDITOR_AREA_ID]?.children?.length).toBeGreaterThan(1);
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

  it('removeEditorGroupFromTree collapses branch when last tab leaves a group', () => {
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
    expect(nodes[DEFAULT_EDITOR_GROUP_ID]?.pixelSize).toBe(800);
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

  it('merges pixel sizes when collapsing a vertical split branch to one group', () => {
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
    expect(nodes[DEFAULT_EDITOR_GROUP_ID]?.pixelSize).toBe(800);
  });
});
