import { describe, expect, it } from 'vitest';

import {
  buildGraphLayoutTab,
  buildWorksheetLayoutTab,
  isGraphLayoutTab,
  isPreviewLayoutTab,
  splitComponentForTab,
} from './layoutTabModel';

describe('layoutTabModel', () => {

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


  it('isGraphLayoutTab and splitComponentForTab', () => {
    const graphTab = buildGraphLayoutTab('events/G.yssbi-event', 'event');
    expect(isGraphLayoutTab(graphTab)).toBe(true);
    expect(isPreviewLayoutTab(graphTab)).toBe(false);
    expect(isPreviewLayoutTab(buildGraphLayoutTab('events/P.yssbi-event', 'event', { pinned: false }))).toBe(true);
    expect(splitComponentForTab(graphTab)).toBe('GraphEditor');
    expect(splitComponentForTab(null)).toBe('GraphEditor');
  });
});
