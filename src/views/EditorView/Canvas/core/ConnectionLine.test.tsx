// @vitest-environment happy-dom

import { act, useLayoutEffect } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ConnectionLine } from './ConnectionLine';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

let resizeCallback: (() => void) | null = null;

vi.mock('@/shared/utils/sashResizeGuard', () => ({
  bindSashAwareResizeObserver: vi.fn((_element: Element, callback: () => void) => {
    resizeCallback = callback;
    return vi.fn();
  }),
}));

vi.mock('@/features/core/viewport', () => ({
  getViewport: () => ({ x: 0, y: 0, scale: 1 }),
  subscribeToViewport: () => vi.fn(),
}));

vi.mock('@/features/core/theme/useTheme', () => ({
  useTheme: () => ({ theme: 'light' }),
}));

vi.mock('@/features/core/canvas/connectPreview', () => ({
  getConnectPreview: () => ({ active: false, startPin: null, worldX: 0, worldY: 0 }),
  subscribeConnectPreview: () => vi.fn(),
}));

vi.mock('./Edge', () => ({ drawEdge: vi.fn() }));

const context = {
  clearRect: vi.fn(),
  save: vi.fn(),
  translate: vi.fn(),
  scale: vi.fn(),
  restore: vi.fn(),
  setTransform: vi.fn(),
};

const defaultRect = {
  x: 0,
  y: 0,
  top: 0,
  right: 320,
  bottom: 180,
  left: 0,
  width: 320,
  height: 180,
  toJSON: () => ({}),
};

function renderConnectionLine(root: Root) {
  act(() => {
    root.render(
      <div>
        <ConnectionLine
          viewportScope={null}
          getPinWorldPos={() => null}
          getCanvasLocalPoint={(x, y) => ({ x, y })}
        />
      </div>,
    );
  });
}

describe('ConnectionLine canvas sizing', () => {
  let host: HTMLDivElement;
  let root: Root;
  let rect: DOMRect;

  beforeEach(() => {
    resizeCallback = null;
    rect = { ...defaultRect } as DOMRect;
    vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockImplementation(() => rect);
    vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue(
      context as unknown as CanvasRenderingContext2D,
    );
    vi.stubGlobal('devicePixelRatio', 2);
    host = document.createElement('div');
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  it('synchronizes a valid size before parent layout effects run', () => {
    let sizeSeenByParent: { width: number; height: number } | null = null;

    function Harness() {
      useLayoutEffect(() => {
        const canvas = host.querySelector('canvas');
        sizeSeenByParent = canvas ? { width: canvas.width, height: canvas.height } : null;
      }, []);

      return (
        <div>
          <ConnectionLine
            viewportScope={null}
            getPinWorldPos={() => null}
            getCanvasLocalPoint={(x, y) => ({ x, y })}
          />
        </div>
      );
    }

    act(() => root.render(<Harness />));

    expect(sizeSeenByParent).toEqual({ width: 640, height: 360 });
  });

  it('ignores transient zero-sized resize observations', () => {
    renderConnectionLine(root);
    const canvas = host.querySelector('canvas')!;

    rect = { ...defaultRect, right: 0, bottom: 0, width: 0, height: 0 } as DOMRect;
    act(() => resizeCallback?.());

    expect(canvas.width).toBe(640);
    expect(canvas.height).toBe(360);
    expect(canvas.style.width).toBe('320px');
    expect(canvas.style.height).toBe('180px');
  });

  it('does not reset the backing store when the observed size is unchanged', () => {
    renderConnectionLine(root);
    const canvas = host.querySelector('canvas')!;
    let width = canvas.width;
    let height = canvas.height;
    const widthSetter = vi.fn((value: number) => { width = value; });
    const heightSetter = vi.fn((value: number) => { height = value; });

    Object.defineProperties(canvas, {
      width: { configurable: true, get: () => width, set: widthSetter },
      height: { configurable: true, get: () => height, set: heightSetter },
    });

    act(() => resizeCallback?.());

    expect(widthSetter).not.toHaveBeenCalled();
    expect(heightSetter).not.toHaveBeenCalled();
  });
});
