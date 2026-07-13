import { beforeEach, describe, expect, it } from 'vitest';
import {
  applyEditorGridMementoWithRepair,
  snapshotEditorGridMemento,
} from './editorGridMemento';
import {
  commitSplitPairSizes,
  computeEditorGridMementoSizes,
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
