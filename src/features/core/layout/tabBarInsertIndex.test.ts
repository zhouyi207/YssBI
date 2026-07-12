import { describe, expect, it } from 'vitest';
import {
  computeTabGapLeft,
  computeTabInsertIndex,
  computeTabShiftOffset,
} from './tabBarInsertIndex';

const metrics = [
  { tabId: 'a', index: 0, left: 0, width: 100 },
  { tabId: 'b', index: 1, left: 100, width: 100 },
  { tabId: 'c', index: 2, left: 200, width: 100 },
  { tabId: 'd', index: 3, left: 300, width: 100 },
] as const;

describe('computeTabInsertIndex', () => {
  it('inserts before the tab whose midpoint is to the right of the pointer', () => {
    expect(computeTabInsertIndex(25, metrics)).toBe(0);
    expect(computeTabInsertIndex(125, metrics)).toBe(1);
    expect(computeTabInsertIndex(325, metrics)).toBe(3);
    expect(computeTabInsertIndex(450, metrics)).toBe(4);
  });
});

describe('computeTabGapLeft', () => {
  it('skips dragged tab width when measuring gap position in the same group', () => {
    expect(computeTabGapLeft(metrics, 3, 'b')).toBe(300);
    expect(computeTabGapLeft(metrics, 0, 'b')).toBe(0);
  });
});

describe('computeTabShiftOffset', () => {
  it('shifts tabs between source and target when dragging right', () => {
    expect(computeTabShiftOffset(2, 1, 3, 80)).toBe(-80);
    expect(computeTabShiftOffset(3, 1, 3, 80)).toBe(0);
  });

  it('shifts tabs between target and source when dragging left', () => {
    expect(computeTabShiftOffset(0, 1, 0, 80)).toBe(80);
    expect(computeTabShiftOffset(1, 1, 0, 80)).toBe(0);
  });
});
