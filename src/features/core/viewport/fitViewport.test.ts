import { describe, expect, it } from 'vitest';
import { fitWorldBounds } from './fitViewport';

describe('fitWorldBounds', () => {
  it('uses 64px default padding, centers the bounds, and does not mutate inputs', () => {
    const bounds = { left: 0, top: 0, right: 100, bottom: 100 };
    const viewportSize = { width: 1000, height: 800 };
    const boundsBefore = { ...bounds };
    const viewportSizeBefore = { ...viewportSize };

    expect(fitWorldBounds(bounds, viewportSize)).toEqual({ x: 250, y: 150, scale: 5 });
    expect(bounds).toEqual(boundsBefore);
    expect(viewportSize).toEqual(viewportSizeBefore);
  });

  it('clamps scale to the shared default limits', () => {
    expect(fitWorldBounds(
      { left: 0, top: 0, right: 10_000, bottom: 10_000 },
      { width: 100, height: 100 },
      { padding: 0 },
    ).scale).toBe(0.1);

    expect(fitWorldBounds(
      { left: 0, top: 0, right: 1, bottom: 1 },
      { width: 1000, height: 1000 },
      { padding: 0 },
    ).scale).toBe(5);
  });

  it('honors custom scale limits', () => {
    expect(fitWorldBounds(
      { left: 0, top: 0, right: 1, bottom: 1 },
      { width: 1000, height: 1000 },
      { minScale: 0.25, maxScale: 2 },
    ).scale).toBe(2);
  });

  it('returns a finite centered viewport for single-point bounds', () => {
    const viewport = fitWorldBounds(
      { left: 20, top: -10, right: 20, bottom: -10 },
      { width: 800, height: 600 },
    );

    expect(viewport).toEqual({ x: 300, y: 350, scale: 5 });
    expect(Object.values(viewport).every(Number.isFinite)).toBe(true);
  });
});
