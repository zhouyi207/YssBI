import { describe, expect, it } from 'vitest';
import { createInitialWorkbenchNodes, DEFAULT_EDITOR_GROUP_ID, EDITOR_AREA_ID } from './workbenchLayoutDefaults';
import {
  applyEqualGridSplit,
  removeEditorGroupFromTree,
  resolveEditorGroupMinSize,
  splitEditorGroupInTree,
} from './editorGridLayout';

describe('editorGridLayout tree ops', () => {
  it('splitEditorGroupInTree forks a sibling group to the right', () => {
    const nodes = createInitialWorkbenchNodes();
    const newId = splitEditorGroupInTree(nodes, DEFAULT_EDITOR_GROUP_ID, 'right', {
      component: 'GraphEditor',
      tabs: [{ id: 'events/foo', component: 'GraphEditor', type: 'event' }],
      activeTabId: 'events/foo',
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
    const nodes = createInitialWorkbenchNodes();
    nodes.sidebar!.pixelSize = 271;
    nodes.panel!.pixelSize = 183;
    nodes.detail!.pixelSize = 319;
    nodes[DEFAULT_EDITOR_GROUP_ID]!.pixelSize = 320;
    const created = splitEditorGroupInTree(nodes, DEFAULT_EDITOR_GROUP_ID, 'right', {
      component: 'GraphEditor',
      tabs: [{ id: 'events/bar', component: 'GraphEditor', type: 'event' }],
      activeTabId: 'events/bar',
    });
    expect(created).toBeTruthy();
    nodes[created!].pixelSize = 480;

    nodes[created!].data!.tabs = [];
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
    // This is the group's width in the row parent. The new col branch has no
    // measured height available, so 640 must not become two 320px heights.
    nodes[DEFAULT_EDITOR_GROUP_ID]!.pixelSize = 640;

    const created = splitEditorGroupInTree(nodes, DEFAULT_EDITOR_GROUP_ID, 'bottom', {
      component: 'GraphEditor',
      tabs: [{ id: 'events/bar', component: 'GraphEditor', type: 'event' }],
      activeTabId: 'events/bar',
    });

    expect(created).toBeTruthy();
    const branchId = nodes[DEFAULT_EDITOR_GROUP_ID]?.parentId;
    expect(branchId).toBeTruthy();
    expect(nodes[branchId!]?.pixelSize).toBeUndefined();
    expect(nodes[branchId!]?.size).toBe(1);
    expect(nodes[DEFAULT_EDITOR_GROUP_ID]?.pixelSize).toBeUndefined();
    expect(nodes[created!]?.pixelSize).toBeUndefined();
  });
});
