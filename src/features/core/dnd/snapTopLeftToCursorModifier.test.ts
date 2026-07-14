import { describe, expect, it } from 'vitest';
import { snapTopLeftToCursor } from './snapTopLeftToCursorModifier';

describe('snapTopLeftToCursor', () => {
  it('offsets transform so overlay top-left follows the pointer', () => {
    const transform = snapTopLeftToCursor({
      activatorEvent: { clientX: 150, clientY: 220 } as PointerEvent,
      active: null,
      activeNodeRect: null,
      draggingNodeRect: {
        top: 200,
        left: 100,
        right: 260,
        bottom: 228,
        width: 160,
        height: 28,
      },
      containerNodeRect: null,
      over: null,
      overlayNodeRect: null,
      scrollableAncestors: [],
      scrollableAncestorRects: [],
      transform: { x: 30, y: 10, scaleX: 1, scaleY: 1 },
      windowRect: null,
    });

    expect(transform).toEqual({
      x: 80,
      y: 30,
      scaleX: 1,
      scaleY: 1,
    });
  });

  it('returns transform unchanged when rects or activator are missing', () => {
    const base = { x: 4, y: 6, scaleX: 1, scaleY: 1 };
    expect(
      snapTopLeftToCursor({
        activatorEvent: null,
        active: null,
        activeNodeRect: null,
        draggingNodeRect: null,
        containerNodeRect: null,
        over: null,
        overlayNodeRect: null,
        scrollableAncestors: [],
        scrollableAncestorRects: [],
        transform: base,
        windowRect: null,
      }),
    ).toBe(base);
  });
});
