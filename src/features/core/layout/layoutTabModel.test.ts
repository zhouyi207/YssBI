import { describe, expect, it } from 'vitest';
import type { LayoutNode } from '@/shared/types';
import type { LayoutTabInput } from './layoutTabModel';
import {
  buildGraphLayoutTab,
  buildWorksheetLayoutTab,
  isGraphLayoutTab,
  normalizeLayoutTab,
  readEditorGroupSnapshot,
  splitComponentForTab,
} from './layoutTabModel';

describe('layoutTabModel', () => {
  it('normalizeLayoutTab fills missing graph tab type', () => {
    const legacy: LayoutTabInput = { id: 'g1', title: 'G', component: 'GraphEditor' };
    expect(normalizeLayoutTab(legacy)).toEqual({
      id: 'g1',
      title: 'G',
      component: 'GraphEditor',
      type: 'event',
    });
  });

  it('buildGraphLayoutTab and buildWorksheetLayoutTab produce typed tabs', () => {
    expect(buildGraphLayoutTab('e1', 'Main', 'event')).toMatchObject({
      type: 'event',
      component: 'GraphEditor',
    });
    expect(buildWorksheetLayoutTab('w1', 'Chart')).toMatchObject({
      type: 'worksheet',
      component: 'WorksheetEditor',
    });
  });

  it('readEditorGroupSnapshot normalizes tabs and params', () => {
    const node: LayoutNode = {
      id: 'editor-a',
      type: 'component',
      parentId: 'root',
      data: {
        component: 'GraphEditor',
        tabs: [buildGraphLayoutTab('g1', 'One', 'function')],
        activeTabId: 'g1',
        params: { selectedNodeIds: ['n1'] },
      },
    };
    const snapshot = readEditorGroupSnapshot(node);
    expect(snapshot?.tabs[0].type).toBe('function');
    expect(snapshot?.selectedNodeIds).toEqual(['n1']);
  });

  it('isGraphLayoutTab and splitComponentForTab', () => {
    const graphTab = buildGraphLayoutTab('g1', 'G', 'event');
    expect(isGraphLayoutTab(graphTab)).toBe(true);
    expect(splitComponentForTab(graphTab)).toBe('GraphEditor');
    expect(splitComponentForTab(null)).toBe('GraphEditor');
  });
});
