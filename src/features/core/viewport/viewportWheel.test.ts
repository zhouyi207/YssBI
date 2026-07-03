import { describe, expect, it } from 'vitest';
import { applyWheelToViewport } from './viewportWheel';

describe('applyWheelToViewport', () => {
  const base = { x: 100, y: 50, scale: 1 };
  const rect = { left: 0, top: 0, width: 800, height: 600 } as DOMRect;

  it('zooms toward cursor with ctrl+wheel', () => {
    const e = {
      ctrlKey: true,
      metaKey: false,
      deltaY: -100,
      clientX: 400,
      clientY: 300,
      shiftKey: false,
      deltaX: 0,
    } as WheelEvent;

    const next = applyWheelToViewport(base, e, rect);
    expect(next.scale).toBeGreaterThan(base.scale);
    expect(next.x).not.toBe(base.x);
    expect(next.y).not.toBe(base.y);
  });

  it('pans without modifier keys', () => {
    const e = {
      ctrlKey: false,
      metaKey: false,
      deltaY: 10,
      deltaX: 5,
      clientX: 100,
      clientY: 100,
      shiftKey: false,
    } as WheelEvent;

    const next = applyWheelToViewport(base, e, rect);
    expect(next.scale).toBe(base.scale);
    expect(next.x).toBe(base.x - 5);
    expect(next.y).toBe(base.y - 10);
  });
});
