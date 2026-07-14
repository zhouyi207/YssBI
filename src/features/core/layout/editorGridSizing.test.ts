import { beforeEach, describe, expect, it } from 'vitest';
import {
  applyEditorGridMementoWithRepair,
  snapshotEditorGridMemento,
} from './editorGridMemento';
import {
  applyEditorGridAddViewSizing,
  applyEditorGridRemoveViewSizing,
  areEditorGridSplitChildrenDistributed,
  clearEditorGridSingletonPixelLocks,
  commitSplitPairSizes,
  computeEditorGridMementoSizes,
  distributeSplitParentRemainingChildren,
  normalizeEditorGridSplitWeights,
  reflowEditorGridLayout,
} from './editorGridSizing';
import {
  createInitialWorkbenchNodes,
  DEFAULT_EDITOR_GROUP_ID,
  EDITOR_AREA_ID,
} from './workbenchLayoutDefaults';
import { splitEditorGroupInTree } from './editorGridLayout';
import { resetEditorTabStore, seedEditorGroupTabs } from './editorTabTestUtils';

describe('editorGridSizing', () => {
  beforeEach(() => {
    resetEditorTabStore();
  });
  it('commitSplitPairSizes stores runtime pixels and normalized flex weights', () => {
    const nodes = createInitialWorkbenchNodes();
    nodes.a = { id: 'a', type: 'component', parentId: EDITOR_AREA_ID, size: 1 };
    nodes.b = { id: 'b', type: 'component', parentId: EDITOR_AREA_ID, size: 1 };

    commitSplitPairSizes(nodes, 'a', 'b', 300, 700);

    expect(nodes.a?.pixelSize).toBe(300);
    expect(nodes.b?.pixelSize).toBe(700);
    expect(nodes.a?.size).toBeCloseTo(0.3);
    expect(nodes.b?.size).toBeCloseTo(0.7);
  });

  it('snapshotEditorGridMemento persists ratio weights without pixelSize', () => {
    const nodes = createInitialWorkbenchNodes();
    nodes[EDITOR_AREA_ID]!.type = 'row';
    nodes[EDITOR_AREA_ID]!.children = [DEFAULT_EDITOR_GROUP_ID, 'editor_group_2'];
    nodes.editor_group_2 = {
      id: 'editor_group_2',
      type: 'component',
      parentId: EDITOR_AREA_ID,
      size: 1,
      data: { component: 'GraphEditor' },
    };
    commitSplitPairSizes(nodes, DEFAULT_EDITOR_GROUP_ID, 'editor_group_2', 400, 600);

    const memento = snapshotEditorGridMemento(nodes, DEFAULT_EDITOR_GROUP_ID);
    const defaultGroup = memento?.nodes.find((node) => node.id === DEFAULT_EDITOR_GROUP_ID);
    const secondGroup = memento?.nodes.find((node) => node.id === 'editor_group_2');

    expect(defaultGroup?.size).toBeCloseTo(0.4);
    expect(secondGroup?.size).toBeCloseTo(0.6);
    expect(defaultGroup).not.toHaveProperty('pixelSize');
    expect(secondGroup).not.toHaveProperty('pixelSize');
  });

  it('hydrates split ratios from memento after restart', () => {
    const nodes = createInitialWorkbenchNodes();
    nodes[EDITOR_AREA_ID]!.type = 'row';
    const created = splitEditorGroupInTree(nodes, DEFAULT_EDITOR_GROUP_ID, 'right', {
      component: 'GraphEditor',
    });
    expect(created).toBeTruthy();
    seedEditorGroupTabs(created!, [{ id: 'events/right', component: 'GraphEditor', type: 'event' }]);

    commitSplitPairSizes(nodes, DEFAULT_EDITOR_GROUP_ID, created!, 250, 750);

    const memento = snapshotEditorGridMemento(nodes, DEFAULT_EDITOR_GROUP_ID);
    expect(memento).toBeTruthy();

    const fresh = createInitialWorkbenchNodes();
    const hydrated = applyEditorGridMementoWithRepair(fresh, memento!);
    const sizes = computeEditorGridMementoSizes(hydrated);

    expect(sizes[DEFAULT_EDITOR_GROUP_ID]).toBeCloseTo(0.25);
    expect(sizes[created!]).toBeCloseTo(0.75);
    expect(hydrated[DEFAULT_EDITOR_GROUP_ID]?.pixelSize).toBeUndefined();
    expect(hydrated[created!]?.pixelSize).toBeUndefined();
  });

  it('persists nested split ratios from flex-only groups without sash drag', () => {
    const nodes = createInitialWorkbenchNodes();
    const rightGroupId = splitEditorGroupInTree(nodes, DEFAULT_EDITOR_GROUP_ID, 'right', {
      component: 'GraphEditor',
    });
    seedEditorGroupTabs(rightGroupId!, [{ id: 'events/right', component: 'GraphEditor', type: 'event' }]);
    const branchId = splitEditorGroupInTree(nodes, DEFAULT_EDITOR_GROUP_ID, 'bottom', {
      component: 'GraphEditor',
    });
    expect(branchId).toBeTruthy();
    seedEditorGroupTabs(branchId!, [{ id: 'events/bottom', component: 'GraphEditor', type: 'event' }]);

    const memento = snapshotEditorGridMemento(nodes, DEFAULT_EDITOR_GROUP_ID);
    expect(memento).toBeTruthy();

    const fresh = createInitialWorkbenchNodes();
    const hydrated = applyEditorGridMementoWithRepair(fresh, memento!);
    expect(hydrated[EDITOR_AREA_ID]?.children?.length).toBeGreaterThan(1);
    expect(hydrated[branchId!]).toBeDefined();
    expect(hydrated[branchId!]?.pixelSize).toBeUndefined();
  });

  it('normalizeEditorGridSplitWeights converts in-memory pixels to ratio-only layout', () => {
    const nodes = createInitialWorkbenchNodes();
    nodes[EDITOR_AREA_ID]!.children = [DEFAULT_EDITOR_GROUP_ID, 'editor_group_2'];
    nodes.editor_group_2 = {
      id: 'editor_group_2',
      type: 'component',
      parentId: EDITOR_AREA_ID,
      size: 1,
      data: { component: 'GraphEditor' },
    };
    commitSplitPairSizes(nodes, DEFAULT_EDITOR_GROUP_ID, 'editor_group_2', 200, 800);

    normalizeEditorGridSplitWeights(nodes);

    expect(nodes[DEFAULT_EDITOR_GROUP_ID]?.pixelSize).toBeUndefined();
    expect(nodes.editor_group_2?.pixelSize).toBeUndefined();
    expect(nodes[DEFAULT_EDITOR_GROUP_ID]?.size).toBeCloseTo(0.2);
    expect(nodes.editor_group_2?.size).toBeCloseTo(0.8);
  });

  it('clearEditorGridSingletonPixelLocks frees a lone editor group after close', () => {
    const nodes = createInitialWorkbenchNodes();
    nodes[DEFAULT_EDITOR_GROUP_ID]!.pixelSize = 640;

    clearEditorGridSingletonPixelLocks(nodes);

    expect(nodes[DEFAULT_EDITOR_GROUP_ID]?.pixelSize).toBeUndefined();
    expect(nodes[DEFAULT_EDITOR_GROUP_ID]?.size).toBe(1);
  });

  it('clearEditorGridSingletonPixelLocks leaves multi-split sash weights intact', () => {
    const nodes = createInitialWorkbenchNodes();
    nodes[EDITOR_AREA_ID]!.children = [DEFAULT_EDITOR_GROUP_ID, 'editor_group_2'];
    nodes.editor_group_2 = {
      id: 'editor_group_2',
      type: 'component',
      parentId: EDITOR_AREA_ID,
      size: 1,
      data: { component: 'GraphEditor' },
    };
    commitSplitPairSizes(nodes, DEFAULT_EDITOR_GROUP_ID, 'editor_group_2', 400, 600);

    clearEditorGridSingletonPixelLocks(nodes);

    expect(nodes[DEFAULT_EDITOR_GROUP_ID]?.pixelSize).toBe(400);
    expect(nodes.editor_group_2?.pixelSize).toBe(600);
  });

  it('applyEditorGridAddViewSizing halves only the target sibling on same-axis insert', () => {
    const nodes = createInitialWorkbenchNodes();
    nodes[EDITOR_AREA_ID]!.children = [DEFAULT_EDITOR_GROUP_ID, 'editor_group_2', 'editor_group_3'];
    nodes[DEFAULT_EDITOR_GROUP_ID]!.size = 0.2;
    nodes.editor_group_2 = {
      id: 'editor_group_2',
      type: 'component',
      parentId: EDITOR_AREA_ID,
      size: 0.8,
      data: { component: 'GraphEditor' },
    };
    nodes.editor_group_3 = {
      id: 'editor_group_3',
      type: 'component',
      parentId: EDITOR_AREA_ID,
      size: 1,
      data: { component: 'GraphEditor' },
    };

    applyEditorGridAddViewSizing(
      nodes,
      EDITOR_AREA_ID,
      'editor_group_2',
      'editor_group_3',
      'same-axis',
    );

    expect(nodes[DEFAULT_EDITOR_GROUP_ID]?.size).toBeCloseTo(0.2);
    expect(nodes.editor_group_2?.size).toBeCloseTo(0.4);
    expect(nodes.editor_group_3?.size).toBeCloseTo(0.4);
  });

  it('areEditorGridSplitChildrenDistributed matches VS Code 2px tolerance on pixel sizes', () => {
    const nodes = createInitialWorkbenchNodes();
    nodes[EDITOR_AREA_ID]!.children = [DEFAULT_EDITOR_GROUP_ID, 'editor_group_2'];
    nodes[DEFAULT_EDITOR_GROUP_ID]!.pixelSize = 499;
    nodes.editor_group_2 = {
      id: 'editor_group_2',
      type: 'component',
      parentId: EDITOR_AREA_ID,
      pixelSize: 501,
      data: { component: 'GraphEditor' },
    };

    expect(areEditorGridSplitChildrenDistributed(nodes, EDITOR_AREA_ID)).toBe(true);

    nodes.editor_group_2.pixelSize = 502;
    expect(areEditorGridSplitChildrenDistributed(nodes, EDITOR_AREA_ID)).toBe(false);
  });

  it('areEditorGridSplitChildrenDistributed uses scaled ratios when pixelSize is absent', () => {
    const nodes = createInitialWorkbenchNodes();
    nodes[EDITOR_AREA_ID]!.children = [DEFAULT_EDITOR_GROUP_ID, 'editor_group_2'];
    nodes[DEFAULT_EDITOR_GROUP_ID]!.size = 0.499;
    nodes.editor_group_2 = {
      id: 'editor_group_2',
      type: 'component',
      parentId: EDITOR_AREA_ID,
      size: 0.501,
      data: { component: 'GraphEditor' },
    };

    expect(areEditorGridSplitChildrenDistributed(nodes, EDITOR_AREA_ID)).toBe(false);

    nodes[DEFAULT_EDITOR_GROUP_ID]!.size = 0.5;
    nodes.editor_group_2.size = 0.5;
    expect(areEditorGridSplitChildrenDistributed(nodes, EDITOR_AREA_ID)).toBe(true);
  });

  it('auto addView distributes when existing siblings are equal before insert', () => {
    const nodes = createInitialWorkbenchNodes();
    nodes[EDITOR_AREA_ID]!.children = [DEFAULT_EDITOR_GROUP_ID, 'editor_group_2', 'editor_group_3'];
    nodes[DEFAULT_EDITOR_GROUP_ID]!.size = 0.5;
    nodes.editor_group_2 = {
      id: 'editor_group_2',
      type: 'component',
      parentId: EDITOR_AREA_ID,
      size: 0.5,
      data: { component: 'GraphEditor' },
    };
    nodes.editor_group_3 = {
      id: 'editor_group_3',
      type: 'component',
      parentId: EDITOR_AREA_ID,
      size: 1,
      data: { component: 'GraphEditor' },
    };

    applyEditorGridAddViewSizing(
      nodes,
      EDITOR_AREA_ID,
      'editor_group_2',
      'editor_group_3',
      'same-axis',
      'auto',
    );

    expect(nodes[DEFAULT_EDITOR_GROUP_ID]?.size).toBeCloseTo(1 / 3);
    expect(nodes.editor_group_2?.size).toBeCloseTo(1 / 3);
    expect(nodes.editor_group_3?.size).toBeCloseTo(1 / 3);
  });

  it('auto addView distributes only when siblings are within 2px', () => {
    const nodes = createInitialWorkbenchNodes();
    nodes[EDITOR_AREA_ID]!.children = [DEFAULT_EDITOR_GROUP_ID, 'editor_group_2', 'editor_group_3'];
    nodes[DEFAULT_EDITOR_GROUP_ID]!.size = 0.499;
    nodes.editor_group_2 = {
      id: 'editor_group_2',
      type: 'component',
      parentId: EDITOR_AREA_ID,
      size: 0.501,
      data: { component: 'GraphEditor' },
    };
    nodes.editor_group_3 = {
      id: 'editor_group_3',
      type: 'component',
      parentId: EDITOR_AREA_ID,
      size: 1,
      data: { component: 'GraphEditor' },
    };

    applyEditorGridAddViewSizing(
      nodes,
      EDITOR_AREA_ID,
      'editor_group_2',
      'editor_group_3',
      'same-axis',
      'auto',
    );

    expect(nodes[DEFAULT_EDITOR_GROUP_ID]?.size).toBeCloseTo(0.499);
    expect(nodes.editor_group_2?.size).toBeCloseTo(0.2505);
    expect(nodes.editor_group_3?.size).toBeCloseTo(0.2505);
  });

  it('applyEditorGridRemoveViewSizing merges removed weight into the left reference sibling', () => {
    const nodes = createInitialWorkbenchNodes();
    nodes[EDITOR_AREA_ID]!.children = [DEFAULT_EDITOR_GROUP_ID, 'editor_group_2', 'editor_group_3'];
    nodes[DEFAULT_EDITOR_GROUP_ID]!.size = 0.2;
    nodes.editor_group_2 = {
      id: 'editor_group_2',
      type: 'component',
      parentId: EDITOR_AREA_ID,
      size: 0.3,
      data: { component: 'GraphEditor' },
    };
    nodes.editor_group_3 = {
      id: 'editor_group_3',
      type: 'component',
      parentId: EDITOR_AREA_ID,
      size: 0.5,
      data: { component: 'GraphEditor' },
    };

    applyEditorGridRemoveViewSizing(nodes, EDITOR_AREA_ID, 'editor_group_2', 'split');
    nodes[EDITOR_AREA_ID]!.children = nodes[EDITOR_AREA_ID]!.children!.filter((id) => id !== 'editor_group_2');
    reflowEditorGridLayout(nodes);

    expect(nodes[DEFAULT_EDITOR_GROUP_ID]?.size).toBeCloseTo(0.5);
    expect(nodes.editor_group_3?.size).toBeCloseTo(0.5);
  });

  it('distributeSplitParentRemainingChildren equalizes survivors after remove', () => {
    const nodes = createInitialWorkbenchNodes();
    nodes[EDITOR_AREA_ID]!.children = [DEFAULT_EDITOR_GROUP_ID, 'editor_group_2'];
    nodes[DEFAULT_EDITOR_GROUP_ID]!.size = 0.25;
    nodes.editor_group_2 = {
      id: 'editor_group_2',
      type: 'component',
      parentId: EDITOR_AREA_ID,
      size: 0.75,
      data: { component: 'GraphEditor' },
    };

    distributeSplitParentRemainingChildren(nodes, EDITOR_AREA_ID);

    expect(nodes[DEFAULT_EDITOR_GROUP_ID]?.size).toBeCloseTo(0.5);
    expect(nodes.editor_group_2?.size).toBeCloseTo(0.5);
  });

  it('reflowEditorGridLayout clears singleton locks then normalizes splits', () => {
    const nodes = createInitialWorkbenchNodes();
    nodes[EDITOR_AREA_ID]!.children = [DEFAULT_EDITOR_GROUP_ID, 'editor_group_2'];
    nodes.editor_group_2 = {
      id: 'editor_group_2',
      type: 'component',
      parentId: EDITOR_AREA_ID,
      size: 1,
      data: { component: 'GraphEditor' },
    };
    commitSplitPairSizes(nodes, DEFAULT_EDITOR_GROUP_ID, 'editor_group_2', 250, 750);

    reflowEditorGridLayout(nodes);

    expect(nodes[DEFAULT_EDITOR_GROUP_ID]?.pixelSize).toBeUndefined();
    expect(nodes.editor_group_2?.pixelSize).toBeUndefined();
    expect(nodes[DEFAULT_EDITOR_GROUP_ID]?.size).toBeCloseTo(0.25);
    expect(nodes.editor_group_2?.size).toBeCloseTo(0.75);
  });

  it('reflowEditorGridLayout invalidates stale maximize snapshot during group maximize', () => {
    const nodes = createInitialWorkbenchNodes();
    nodes[EDITOR_AREA_ID]!.children = [DEFAULT_EDITOR_GROUP_ID, 'editor_group_2'];
    nodes.editor_group_2 = {
      id: 'editor_group_2',
      type: 'component',
      parentId: EDITOR_AREA_ID,
      pixelSize: 400,
      data: { component: 'GraphEditor' },
    };
    nodes[DEFAULT_EDITOR_GROUP_ID]!.pixelSize = 400;
    nodes[EDITOR_AREA_ID]!.data = {
      maximizedGroupId: DEFAULT_EDITOR_GROUP_ID,
      restoredGridSizes: { [DEFAULT_EDITOR_GROUP_ID]: 400, editor_group_2: 400 },
    };

    reflowEditorGridLayout(nodes);

    expect(nodes[DEFAULT_EDITOR_GROUP_ID]?.pixelSize).toBeUndefined();
    expect(nodes[EDITOR_AREA_ID]?.data?.restoredGridSizes).toBeUndefined();
    expect(nodes[EDITOR_AREA_ID]?.data?.maximizedGroupId).toBe(DEFAULT_EDITOR_GROUP_ID);
  });
});
