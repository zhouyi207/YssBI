import { describe, expect, it } from 'vitest';
import type { LayoutNode } from '@/shared/types';
import type { LayoutTabInput } from './layoutTabModel';
import {
  buildGraphLayoutTab,
  buildWorksheetLayoutTab,
  isGraphLayoutTab,
  isPreviewLayoutTab,
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
    expect(buildGraphLayoutTab('events/Main.yssbi-event', 'Main', 'event')).toMatchObject({
      id: 'events/Main.yssbi-event',
      type: 'event',
      component: 'GraphEditor',
    });
    expect(
      buildGraphLayoutTab('untitled:function:Untitled-1', 'Draft', 'function'),
    ).toMatchObject({
      id: 'untitled:function:Untitled-1',
      type: 'function',
    });
    expect(buildWorksheetLayoutTab('w1')).toMatchObject({
      id: 'w1',
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
        tabs: [buildGraphLayoutTab('functions/One.yssbi-function', 'One', 'function')],
        activeTabId: 'functions/One.yssbi-function',
        params: { selectedNodeIds: ['n1'] },
      },
    };
    const snapshot = readEditorGroupSnapshot(node);
    expect(snapshot?.tabs[0].type).toBe('function');
    expect(snapshot?.selectedNodeIds).toEqual(['n1']);
  });

  it('isGraphLayoutTab and splitComponentForTab', () => {
    const graphTab = buildGraphLayoutTab('events/G.yssbi-event', 'G', 'event');
    expect(isGraphLayoutTab(graphTab)).toBe(true);
    expect(isPreviewLayoutTab(graphTab)).toBe(false);
    expect(isPreviewLayoutTab(buildGraphLayoutTab('events/P.yssbi-event', 'P', 'event', { pinned: false }))).toBe(true);
    expect(splitComponentForTab(graphTab)).toBe('GraphEditor');
    expect(splitComponentForTab(null)).toBe('GraphEditor');
  });
});
