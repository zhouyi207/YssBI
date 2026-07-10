import { describe, expect, it } from 'vitest';
import { computeFlexSplitSizes, isFlexSplitPair } from './splitViewSizing';
import type { LayoutNode } from '@/shared/types/ui';

const flexNode = (id: string): LayoutNode => ({
  id,
  type: 'component',
  parentId: 'branch',
  size: 1,
  data: { component: 'GraphEditor', tabs: [] },
});

const fixedNode = (id: string, pixelSize: number): LayoutNode => ({
  ...flexNode(id),
  pixelSize,
});

describe('isFlexSplitPair', () => {
  it('true when both siblings are flex-sized', () => {
    expect(isFlexSplitPair(flexNode('a'), flexNode('b'))).toBe(true);
  });

  it('false when either has pixelSize', () => {
    expect(isFlexSplitPair(fixedNode('a', 400), flexNode('b'))).toBe(false);
  });
});

describe('computeFlexSplitSizes', () => {
  it('allocates pointer delta between both siblings', () => {
    const pair = { beforeId: 'a', afterId: 'b', beforeStart: 600, afterStart: 400 };
    expect(computeFlexSplitSizes(pair, 50)).toEqual({ beforeSize: 650, afterSize: 350 });
    expect(computeFlexSplitSizes(pair, -100)).toEqual({ beforeSize: 500, afterSize: 500 });
  });

  it('respects minimum sizes', () => {
    const pair = { beforeId: 'a', afterId: 'b', beforeStart: 300, afterStart: 300 };
    expect(computeFlexSplitSizes(pair, -500, 200, 200)).toEqual({ beforeSize: 200, afterSize: 400 });
    expect(computeFlexSplitSizes(pair, 500, 200, 200)).toEqual({ beforeSize: 400, afterSize: 200 });
  });

  it('preserves the available total when it is smaller than both minimums', () => {
    const pair = { beforeId: 'a', afterId: 'b', beforeStart: 100, afterStart: 100 };
    expect(computeFlexSplitSizes(pair, 500, 200, 200)).toEqual({ beforeSize: 100, afterSize: 100 });
  });
});
