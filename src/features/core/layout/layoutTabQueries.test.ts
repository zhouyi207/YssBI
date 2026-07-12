import { describe, expect, it } from 'vitest';
import type { LayoutTree } from '@/shared/types';
import {
  getActiveLayoutTab,
  getLayoutTabById,
  locateLayoutTab,
  resolveEditorGroupId,
  resolveEditorTargetGroupId,
} from './layoutTabQueries';

const mockNodes: LayoutTree = {
  editorA: {
    id: 'editorA',
    type: 'component',
    parentId: 'root',
    data: {
      component: 'GraphEditor',
      tabs: [
        { id: 'g1', component: 'GraphEditor', type: 'event' },
        { id: 'g2', component: 'GraphEditor', type: 'function' },
      ],
      activeTabId: 'g1',
    },
  },
  editorB: {
    id: 'editorB',
    type: 'component',
    parentId: 'root',
    data: {
      component: 'GraphEditor',
      tabs: [{ id: 'g3', component: 'GraphEditor', type: 'event' }],
      activeTabId: 'g3',
    },
  },
  panel: {
    id: 'panel',
    type: 'component',
    parentId: 'root',
    data: {
      component: 'PanelPart',
      tabs: [],
      activeTabId: undefined,
    },
  },
};

describe('layoutTabQueries', () => {
  it('finds a tab globally by id', () => {
    expect(getLayoutTabById('g2', mockNodes)).toEqual({
      nodeId: 'editorA',
      tab: mockNodes.editorA.data!.tabs![1],
    });
    expect(getLayoutTabById('missing', mockNodes)).toBeNull();
  });

  it('locates a tab in a specific node', () => {
    expect(locateLayoutTab('g3', 'editorB', mockNodes)).toEqual({
      nodeId: 'editorB',
      tab: mockNodes.editorB.data!.tabs![0],
    });
    expect(locateLayoutTab('g3', 'editorA', mockNodes)).toBeNull();
  });

  it('falls back to global search when nodeId is omitted', () => {
    expect(locateLayoutTab('g1', undefined, mockNodes)?.nodeId).toBe('editorA');
  });

  it('returns active tab for a group', () => {
    expect(getActiveLayoutTab('editorA', mockNodes)).toEqual({
      activeTabId: 'g1',
      tab: mockNodes.editorA.data!.tabs![0],
    });
    expect(getActiveLayoutTab('panel', mockNodes)).toBeNull();
  });

  it('returns null when activeTabId points to a missing tab', () => {
    const broken: LayoutTree = {
      editor: {
        id: 'editor',
        type: 'component',
        parentId: null,
        data: {
          component: 'GraphEditor',
          tabs: [],
          activeTabId: 'ghost',
        },
      },
    };
    expect(getActiveLayoutTab('editor', broken)).toBeNull();
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
        data: { component: 'Sidebar', isFixed: true, tabs: [], activeTabId: undefined },
      },
    };
    expect(
      resolveEditorTargetGroupId(undefined, tree, {
        activeEditorGroupId: null,
      }),
    ).toBe('editorA');
  });
});
