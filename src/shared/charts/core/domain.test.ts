import { describe, expect, it } from 'vitest';
import { DEFAULT_CARTESIAN_MARGIN } from './margins';
import { paddedNumericDomain, resolveChartBox } from './domain';

describe('resolveChartBox', () => {
  it('resolves plot dimensions and rejects non-positive plot areas', () => {
    expect(resolveChartBox(640, 320, DEFAULT_CARTESIAN_MARGIN)).toEqual({
      width: 640,
      height: 320,
      plotWidth: 560,
      plotHeight: 260,
    });

    expect(resolveChartBox(80, 320, DEFAULT_CARTESIAN_MARGIN)).toBeNull();
    expect(resolveChartBox(640, 60, DEFAULT_CARTESIAN_MARGIN)).toBeNull();
  });
});

describe('paddedNumericDomain', () => {
  it('pads constant finite values and defaults an empty domain', () => {
    expect(paddedNumericDomain([4, 4], 0.06, 1)).toEqual([3, 5]);
    expect(
      paddedNumericDomain([Number.NEGATIVE_INFINITY, 4, Number.NaN], 0.06, 1),
    ).toEqual([3, 5]);
    expect(paddedNumericDomain([], 0.06, 1)).toEqual([0, 1]);
  });
});
