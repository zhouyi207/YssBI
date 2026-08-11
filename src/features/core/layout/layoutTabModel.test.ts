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
      component: 'GraphEditor',
      type: 'event',
    });
  });

  it('buildGraphLayoutTab and buildWorksheetLayoutTab produce typed tabs', () => {
    expect(buildGraphLayoutTab('events/Main.yssbi-event', 'event')).toMatchObject({
      id: 'events/Main.yssbi-event',
      type: 'event',
      component: 'GraphEditor',
    });
    expect(buildGraphLayoutTab('functions/Helper.yssbi-function', 'function')).toMatchObject({
      id: 'functions/Helper.yssbi-function',
      type: 'function',
    });
    const worksheetPath = 'worksheets/Opaque Path With Spaces.yssbi-worksheet';
    expect(buildWorksheetLayoutTab(worksheetPath)).toMatchObject({
      id: worksheetPath,
      type: 'worksheet',
      component: 'WorksheetEditor',
    });
  });

  it('readEditorGroupSnapshot returns stable group identity', () => {
    const node: LayoutNode = {
      id: 'editor-a',
      type: 'component',
      parentId: 'root',
      data: {
        component: 'GraphEditor',
      },
    };
    expect(readEditorGroupSnapshot(node)).toEqual({ id: 'editor-a' });
  });

  it('isGraphLayoutTab and splitComponentForTab', () => {
    const graphTab = buildGraphLayoutTab('events/G.yssbi-event', 'event');
    expect(isGraphLayoutTab(graphTab)).toBe(true);
    expect(isPreviewLayoutTab(graphTab)).toBe(false);
    expect(isPreviewLayoutTab(buildGraphLayoutTab('events/P.yssbi-event', 'event', { pinned: false }))).toBe(true);
    expect(splitComponentForTab(graphTab)).toBe('GraphEditor');
    expect(splitComponentForTab(null)).toBe('GraphEditor');
  });
});
