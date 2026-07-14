import { beforeEach, describe, expect, it } from 'vitest';
import type { LayoutTree } from '@/shared/types';
import {
  getActiveLayoutTab,
  getLayoutTabById,
  locateLayoutTab,
  resolveEditorGroupId,
  resolveEditorTargetGroupId,
} from './layoutTabQueries';
import { resetEditorTabStore, seedEditorGroupTabs } from './editorTabTestUtils';

const mockNodes: LayoutTree = {
  editorA: {
    id: 'editorA',
    type: 'component',
    parentId: 'root',
    data: { component: 'GraphEditor' },
  },
  editorB: {
    id: 'editorB',
    type: 'component',
    parentId: 'root',
    data: { component: 'GraphEditor' },
  },
  panel: {
    id: 'panel',
    type: 'component',
    parentId: 'root',
    data: { component: 'PanelPart' },
  },
};

const tabG1 = { id: 'g1', component: 'GraphEditor' as const, type: 'event' as const };
const tabG2 = { id: 'g2', component: 'GraphEditor' as const, type: 'function' as const };
const tabG3 = { id: 'g3', component: 'GraphEditor' as const, type: 'event' as const };

describe('layoutTabQueries', () => {
  beforeEach(() => {
    resetEditorTabStore();
    seedEditorGroupTabs('editorA', [tabG1, tabG2], 'g1');
    seedEditorGroupTabs('editorB', [tabG3], 'g3');
  });

  it('finds a tab globally by id', () => {
    expect(getLayoutTabById('g2')).toEqual({
      nodeId: 'editorA',
      tab: tabG2,
    });
    expect(getLayoutTabById('missing')).toBeNull();
  });

  it('does not read tabs embedded in layout nodes', () => {
    expect(getLayoutTabById('embedded')).toBeNull();
  });

  it('locates a tab in a specific node', () => {
    expect(locateLayoutTab('g3', 'editorB', mockNodes)).toEqual({
      nodeId: 'editorB',
      tab: tabG3,
    });
    expect(locateLayoutTab('g3', 'editorA', mockNodes)).toBeNull();
  });

  it('falls back to global search when nodeId is omitted', () => {
    expect(locateLayoutTab('g1', undefined, mockNodes)?.nodeId).toBe('editorA');
  });

  it('returns active tab for a group', () => {
    expect(getActiveLayoutTab('editorA', mockNodes)).toEqual({
      activeTabId: 'g1',
      tab: tabG1,
    });
    expect(getActiveLayoutTab('panel', mockNodes)).toBeNull();
  });

  it('returns null when activeTabId points to a missing tab', () => {
    resetEditorTabStore();
    seedEditorGroupTabs('editor', [], 'ghost');

    expect(getActiveLayoutTab('editor', mockNodes)).toBeNull();
  });

  it('resolves editor group id with fallbacks', () => {
    expect(resolveEditorGroupId(undefined, { activeEditorGroupId: 'editorA' })).toBe(
      'editorA',
    );
    expect(resolveEditorGroupId(null, { activeEditorGroupId: null })).toBe(null);
    expect(resolveEditorGroupId('explicit', { activeEditorGroupId: 'editorA' })).toBe(
      'explicit',
    );
  });

  it('resolveEditorTargetGroupId skips fixed chrome nodes', () => {
    const tree: LayoutTree = {
      ...mockNodes,
      sidebar: {
        id: 'sidebar',
        type: 'component',
        parentId: 'root',
        data: { component: 'Sidebar', isFixed: true },
      },
    };
    expect(
      resolveEditorTargetGroupId(undefined, tree, {
        activeEditorGroupId: null,
      }),
    ).toBe('editorA');
  });
});
