import { describe, expect, it } from 'vitest';
import { createInitialWorkbenchNodes, DEFAULT_EDITOR_GROUP_ID, EDITOR_AREA_ID } from './workbenchLayoutDefaults';
import {
  applyEqualGridSplit,
  removeEditorGroupFromTree,
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

  it('removeEditorGroupFromTree collapses branch when last tab leaves a group', () => {
    const nodes = createInitialWorkbenchNodes();
    const created = splitEditorGroupInTree(nodes, DEFAULT_EDITOR_GROUP_ID, 'right', {
      component: 'GraphEditor',
      tabs: [{ id: 'events/bar', component: 'GraphEditor', type: 'event' }],
      activeTabId: 'events/bar',
    });
    expect(created).toBeTruthy();

    nodes[created!].data!.tabs = [];
    const { removed, nextActiveGroupId } = removeEditorGroupFromTree(nodes, created!);
    expect(removed).toBe(true);
    expect(nodes[created!]).toBeUndefined();
    expect(nextActiveGroupId).toBe(DEFAULT_EDITOR_GROUP_ID);
  });
});
