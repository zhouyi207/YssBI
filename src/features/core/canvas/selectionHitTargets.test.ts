import { describe, expect, it } from 'vitest';
import { hitTestSelection } from './selectionHitTargets';
import type { SelectionHitTarget } from './selectionHitTargets';
import {
  selectionScreenRect,
  selectionSessionMoved,
  startSelectionSession,
  endSelectionSession,
  getSelectionSession,
  getSelectionPreviewIds,
  abortSelectionSession,
} from './selectionSession';

describe('hitTestSelection', () => {
  const targets: SelectionHitTarget[] = [
    { id: 'a', left: 10, right: 50, top: 10, bottom: 50 },
    { id: 'b', left: 100, right: 140, top: 10, bottom: 50 },
  ];

  it('returns ids intersecting the rect', () => {
    expect(hitTestSelection(targets, { x1: 0, y1: 0, x2: 60, y2: 60 })).toEqual(['a']);
    expect(hitTestSelection(targets, { x1: 90, y1: 0, x2: 150, y2: 60 })).toEqual(['b']);
    expect(hitTestSelection(targets, { x1: 0, y1: 0, x2: 200, y2: 60 })).toEqual(['a', 'b']);
  });

  it('returns empty when nothing intersects', () => {
    expect(hitTestSelection(targets, { x1: 200, y1: 200, x2: 300, y2: 300 })).toEqual([]);
  });
});

describe('selectionSession', () => {
  it('tracks screen rect and movement', () => {
    startSelectionSession({ groupId: 'g1', startX: 10, startY: 20, preserveSelection: false });
    const session = getSelectionSession();
    expect(session.active).toBe(true);
    if (!session.active) return;

    expect(selectionScreenRect({ ...session, currentX: 30, currentY: 50 })).toEqual({
      x1: 10,
      y1: 20,
      x2: 30,
      y2: 50,
    });
    expect(selectionSessionMoved({ ...session, currentX: 11, currentY: 20 }, 3)).toBe(false);
    expect(selectionSessionMoved({ ...session, currentX: 20, currentY: 20 }, 3)).toBe(true);

    endSelectionSession();
    expect(getSelectionSession().active).toBe(false);
    expect(getSelectionPreviewIds()).toEqual([]);
  });

  it('abortSelectionSession ends an active session', () => {
    startSelectionSession({ groupId: 'g1', startX: 0, startY: 0, preserveSelection: false });
    abortSelectionSession();
    expect(getSelectionSession().active).toBe(false);
  });
});
