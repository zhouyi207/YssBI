import { describe, expect, it } from 'vitest';
import { applyWheelZoomToViewport, isCanvasWheelZoomGesture } from './canvasWheelZoom';

describe('canvasWheelZoom', () => {
  const base = { x: 100, y: 50, scale: 1 };
  const rect = { left: 0, top: 0, width: 800, height: 600 } as DOMRect;

  it('detects ctrl/meta wheel as zoom gesture', () => {
    expect(isCanvasWheelZoomGesture({ ctrlKey: true, metaKey: false })).toBe(true);
    expect(isCanvasWheelZoomGesture({ ctrlKey: false, metaKey: true })).toBe(true);
    expect(isCanvasWheelZoomGesture({ ctrlKey: false, metaKey: false })).toBe(false);
  });

  it('zooms toward cursor with ctrl+wheel', () => {
    const e = {
      ctrlKey: true,
      metaKey: false,
      deltaY: -100,
      clientX: 400,
      clientY: 300,
    } as WheelEvent;

    const next = applyWheelZoomToViewport(base, e, rect);
    expect(next.scale).toBeGreaterThan(base.scale);
    expect(next.x).not.toBe(base.x);
    expect(next.y).not.toBe(base.y);
  });

  it('ignores plain wheel without modifier', () => {
    const e = {
      ctrlKey: false,
      metaKey: false,
      deltaY: 100,
      clientX: 100,
      clientY: 100,
    } as WheelEvent;

    expect(applyWheelZoomToViewport(base, e, rect)).toEqual(base);
  });
});
