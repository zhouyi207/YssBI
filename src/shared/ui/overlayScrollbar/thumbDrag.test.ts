// @vitest-environment happy-dom

import { afterEach, describe, expect, it, vi } from 'vitest';
import type { ScrollbarAxisMetrics } from './metrics';
import {
  applyThumbDragPosition,
  beginThumbDragSession,
  bindScrollbarThumbDrag,
  resolveThumbDragOffset,
  withInstantViewportScroll,
} from './thumbDrag';

const metrics: ScrollbarAxisMetrics = {
  maxScroll: 1000,
  trackLength: 200,
  thumbSize: 40,
  thumbOffset: 80,
  scrollRatio: 1000 / 160,
};

function pointerEvent(
  type: string,
  init: PointerEventInit & { pointerId?: number },
): PointerEvent {
  return new PointerEvent(type, {
    bubbles: true,
    cancelable: true,
    pointerId: 1,
    ...init,
  });
}

function createScrollViewport(axis: 'x' | 'y') {
  const viewport = document.createElement('div');
  const scrollSize = 2000;
  const clientSize = 500;

  if (axis === 'y') {
    Object.defineProperty(viewport, 'scrollHeight', { value: scrollSize, configurable: true });
    Object.defineProperty(viewport, 'clientHeight', { value: clientSize, configurable: true });
    Object.defineProperty(viewport, 'scrollTop', { value: 0, writable: true, configurable: true });
  } else {
    Object.defineProperty(viewport, 'scrollWidth', { value: scrollSize, configurable: true });
    Object.defineProperty(viewport, 'clientWidth', { value: clientSize, configurable: true });
    Object.defineProperty(viewport, 'scrollLeft', { value: 0, writable: true, configurable: true });
  }

  return viewport;
}

describe('overlayScrollbar thumb drag', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  describe('beginThumbDragSession', () => {
    it('creates a frozen session from primary-button pointer down', () => {
      const event = pointerEvent('pointerdown', { clientY: 120, button: 0, pointerId: 7 });
      const session = beginThumbDragSession('y', event, metrics);

      expect(session).toEqual({
        axis: 'y',
        pointerId: 7,
        originPointer: 120,
        originThumbOffset: 80,
        metrics,
      });
    });

    it('rejects non-primary button presses', () => {
      const event = pointerEvent('pointerdown', { clientY: 120, button: 1, pointerId: 1 });
      expect(beginThumbDragSession('y', event, metrics)).toBeNull();
    });
  });

  describe('resolveThumbDragOffset', () => {
    it('maps vertical pointer movement to thumb offset with clamping', () => {
      const session = beginThumbDragSession(
        'y',
        pointerEvent('pointerdown', { clientY: 100, button: 0 }),
        metrics,
      )!;

      expect(resolveThumbDragOffset(session, pointerEvent('pointermove', { clientY: 116 }))).toBe(96);
      expect(resolveThumbDragOffset(session, pointerEvent('pointermove', { clientY: 400 }))).toBe(160);
      expect(resolveThumbDragOffset(session, pointerEvent('pointermove', { clientY: -100 }))).toBe(0);
    });

    it('maps horizontal pointer movement to thumb offset', () => {
      const session = beginThumbDragSession(
        'x',
        pointerEvent('pointerdown', { clientX: 50, button: 0 }),
        metrics,
      )!;

      expect(resolveThumbDragOffset(session, pointerEvent('pointermove', { clientX: 66 }))).toBe(96);
    });
  });

  describe('applyThumbDragPosition', () => {
    it('updates thumb top and viewport scrollTop together', () => {
      const thumb = document.createElement('div');
      const viewport = createScrollViewport('y');

      applyThumbDragPosition({
        axis: 'y',
        thumbEl: thumb,
        viewport,
        thumbOffset: 96,
        metrics,
      });

      expect(thumb.style.top).toBe('96px');
      expect(viewport.scrollTop).toBe(600);
    });

    it('updates thumb left and viewport scrollLeft together', () => {
      const thumb = document.createElement('div');
      const viewport = createScrollViewport('x');

      applyThumbDragPosition({
        axis: 'x',
        thumbEl: thumb,
        viewport,
        thumbOffset: 40,
        metrics,
      });

      expect(thumb.style.left).toBe('40px');
      expect(viewport.scrollLeft).toBe(250);
    });
  });

  describe('withInstantViewportScroll', () => {
    it('forces instant scroll behavior during programmatic updates', () => {
      const viewport = document.createElement('div');
      viewport.style.scrollBehavior = 'smooth';

      withInstantViewportScroll(viewport, () => {
        expect(viewport.style.scrollBehavior).toBe('auto');
      });

      expect(viewport.style.scrollBehavior).toBe('smooth');
    });
  });

  describe('bindScrollbarThumbDrag', () => {
    it('keeps thumb and viewport in sync while dragging on the track host', () => {
      const captureHost = document.createElement('div');
      const thumb = document.createElement('div');
      const viewport = createScrollViewport('y');
      document.body.append(captureHost, thumb, viewport);

      const session = beginThumbDragSession(
        'y',
        pointerEvent('pointerdown', { clientY: 100, button: 0, pointerId: 3 }),
        metrics,
      )!;

      const dragging: boolean[] = [];
      const cleanup = bindScrollbarThumbDrag({
        captureHost,
        thumbEl: thumb,
        viewport,
        session,
        onDraggingChange: (draggingState) => dragging.push(draggingState),
      });

      expect(captureHost.hasPointerCapture(3)).toBe(true);
      expect(dragging).toEqual([true]);
      expect(viewport.style.scrollBehavior).toBe('auto');

      captureHost.dispatchEvent(pointerEvent('pointermove', { clientY: 116, pointerId: 3 }));
      expect(thumb.style.top).toBe('96px');
      expect(viewport.scrollTop).toBe(600);

      captureHost.dispatchEvent(pointerEvent('pointerup', { clientY: 116, pointerId: 3 }));
      expect(dragging).toEqual([true, false]);
      expect(captureHost.hasPointerCapture(3)).toBe(false);
      expect(viewport.style.scrollBehavior).toBe('');

      cleanup();
    });

    it('supports horizontal drag sessions on the track host', () => {
      const captureHost = document.createElement('div');
      const thumb = document.createElement('div');
      const viewport = createScrollViewport('x');
      document.body.append(captureHost, thumb, viewport);

      const session = beginThumbDragSession(
        'x',
        pointerEvent('pointerdown', { clientX: 80, button: 0, pointerId: 5 }),
        metrics,
      )!;

      bindScrollbarThumbDrag({
        captureHost,
        thumbEl: thumb,
        viewport,
        session,
      });

      captureHost.dispatchEvent(pointerEvent('pointermove', { clientX: 96, pointerId: 5 }));
      expect(thumb.style.left).toBe('96px');
      expect(viewport.scrollLeft).toBe(600);
    });

    it('ignores pointer events from other pointers', () => {
      const captureHost = document.createElement('div');
      const thumb = document.createElement('div');
      const viewport = createScrollViewport('y');
      document.body.append(captureHost, thumb, viewport);

      const session = beginThumbDragSession(
        'y',
        pointerEvent('pointerdown', { clientY: 100, button: 0, pointerId: 2 }),
        metrics,
      )!;

      bindScrollbarThumbDrag({
        captureHost,
        thumbEl: thumb,
        viewport,
        session,
      });

      captureHost.dispatchEvent(pointerEvent('pointermove', { clientY: 140, pointerId: 9 }));
      expect(thumb.style.top).toBe('');
      expect(viewport.scrollTop).toBe(0);
    });

    it('cleans up listeners and pointer capture when aborted early', () => {
      const captureHost = document.createElement('div');
      const thumb = document.createElement('div');
      const viewport = createScrollViewport('y');
      document.body.append(captureHost, thumb, viewport);

      const onDraggingChange = vi.fn();
      const session = beginThumbDragSession(
        'y',
        pointerEvent('pointerdown', { clientY: 100, button: 0, pointerId: 4 }),
        metrics,
      )!;

      const cleanup = bindScrollbarThumbDrag({
        captureHost,
        thumbEl: thumb,
        viewport,
        session,
        onDraggingChange,
      });

      cleanup();

      expect(onDraggingChange).toHaveBeenLastCalledWith(false);
      expect(captureHost.hasPointerCapture(4)).toBe(false);

      captureHost.dispatchEvent(pointerEvent('pointermove', { clientY: 140, pointerId: 4 }));
      expect(thumb.style.top).toBe('');
      expect(viewport.scrollTop).toBe(0);
    });
  });
});
