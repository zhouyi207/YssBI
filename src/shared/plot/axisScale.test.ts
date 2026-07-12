import { describe, expect, it } from 'vitest';
import {
  columnAxisKindFromType,
  createColumnAxisScale,
  mapColumnAxisValue,
  numericColumnAxisTicks,
} from './axisScale';

describe('axisScale', () => {
  it('columnAxisKindFromType maps parallel axis types', () => {
    expect(columnAxisKindFromType('number')).toBe('numeric');
    expect(columnAxisKindFromType('string')).toBe('category');
  });

  it('createColumnAxisScale builds numeric scale with padding', () => {
    const axis = createColumnAxisScale('numeric', [1, 2, 3, 4], [100, 0]);
    expect(axis.kind).toBe('numeric');
    expect(mapColumnAxisValue(axis, 2)).toBeGreaterThan(0);
    expect(mapColumnAxisValue(axis, 2)).toBeLessThan(100);
    expect(numericColumnAxisTicks(axis, 3).length).toBeGreaterThan(0);
  });

  it('createColumnAxisScale builds category scale', () => {
    const axis = createColumnAxisScale('category', ['a', 'b', 'a', null], [80, 0]);
    expect(axis.kind).toBe('category');
    expect(mapColumnAxisValue(axis, 'a')).toBeDefined();
    expect(mapColumnAxisValue(axis, 'missing')).toBeUndefined();
    expect(numericColumnAxisTicks(axis)).toEqual([]);
  });

  it('mapColumnAxisValue rejects non-finite numeric input', () => {
    const axis = createColumnAxisScale('numeric', [0, 10], [50, 0]);
    expect(mapColumnAxisValue(axis, 'not-a-number')).toBeUndefined();
    expect(mapColumnAxisValue(axis, null)).toBeUndefined();
  });
});
