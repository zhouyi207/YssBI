// @vitest-environment happy-dom

import { act, createElement, useRef } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';

import type { ViewportScope } from '@/features/core/viewport';
import {
  getViewport,
  resetLiveViewports,
} from '@/features/core/viewport';
import { useCanvasWheelZoom } from './useCanvasWheelZoom';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const viewportScope: ViewportScope = {
  groupId: 'group-1',
  graphPath: 'events/Main.yssbi-event',
};

describe('useCanvasWheelZoom', () => {
  let host: HTMLDivElement;
  let root: Root;

  function Harness({ interactive }: { interactive: boolean }) {
    const canvasRef = useRef<HTMLDivElement>(null);
    useCanvasWheelZoom(canvasRef, viewportScope, interactive);
    return createElement('div', { ref: canvasRef });
  }

  function dispatchWheel(canvas: Element): void {
    canvas.dispatchEvent(new WheelEvent('wheel', {
      bubbles: true,
      cancelable: true,
      clientX: 100,
      clientY: 100,
      deltaY: -100,
    }));
  }

  beforeEach(() => {
    resetLiveViewports(viewportScope);
    host = document.createElement('div');
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => root.unmount());
    resetLiveViewports(viewportScope);
    host.remove();
  });

  it('rebinds wheel zoom across inactive and active panel transitions', async () => {
    await act(async () => {
      root.render(createElement(Harness, { interactive: true }));
      await Promise.resolve();
    });

    const canvas = host.firstElementChild;
    expect(canvas).not.toBeNull();

    act(() => dispatchWheel(canvas!));
    const activeScale = getViewport(viewportScope).scale;
    expect(activeScale).toBeGreaterThan(1);

    await act(async () => {
      root.render(createElement(Harness, { interactive: false }));
      await Promise.resolve();
    });

    act(() => dispatchWheel(canvas!));
    expect(getViewport(viewportScope).scale).toBe(activeScale);

    await act(async () => {
      root.render(createElement(Harness, { interactive: true }));
      await Promise.resolve();
    });

    act(() => dispatchWheel(canvas!));
    expect(getViewport(viewportScope).scale).toBeGreaterThan(activeScale);
  });
});
