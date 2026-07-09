// @vitest-environment happy-dom

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { attachSashDrag } from './sashResizeLogic';
import { useLayoutStore } from '@/features/core/layout/layoutStore';

vi.mock('@/features/core/layout/workbenchLayoutService', () => ({
  persistWorkbenchLayoutDebounced: vi.fn(),
  persistEditorGridDebounced: vi.fn(),
  togglePanelMaximized: vi.fn(),
}));

function mockRect(width: number, height: number): DOMRect {
  return {
    width,
    height,
    top: 0,
    left: 0,
    right: width,
    bottom: height,
    x: 0,
    y: 0,
    toJSON: () => ({}),
  } as DOMRect;
}

function layoutDiv(width: number, height: number): HTMLDivElement {
  const el = document.createElement('div');
  el.getBoundingClientRect = () => mockRect(width, height);
  return el;
}

describe('attachSashDrag store commits', () => {
  let resizeSpy: ReturnType<typeof vi.spyOn>;
  let detach: (() => void) | null = null;

  beforeEach(() => {
    useLayoutStore.setState({
      nodes: {
        sidebar: {
          id: 'sidebar',
          type: 'component',
          parentId: 'root',
          pixelSize: 260,
          minSize: 240,
          data: { component: 'Sidebar', visible: true },
        },
        center: {
          id: 'center',
          type: 'col',
          parentId: 'root',
          size: 1,
          children: [],
        },
      },
    });

    resizeSpy = vi.spyOn(useLayoutStore.getState(), 'resizeNode');
  });

  afterEach(() => {
    detach?.();
    detach = null;
    resizeSpy.mockRestore();
    vi.clearAllMocks();
  });

  it('commits resizeNode once on mouseup; no store writes during mousemove', () => {
    const beforeEl = layoutDiv(260, 600);
    const afterEl = layoutDiv(800, 600);
    const sash = document.createElement('div');

    detach = attachSashDrag(sash, {
      orientation: 'row',
      beforeNodeId: 'sidebar',
      afterNodeId: 'center',
      getBeforeEl: () => beforeEl,
      getAfterEl: () => afterEl,
    });

    sash.dispatchEvent(new MouseEvent('mousedown', { clientX: 100, clientY: 300, bubbles: true }));
    window.dispatchEvent(new MouseEvent('mousemove', { clientX: 120, clientY: 300, bubbles: true }));
    window.dispatchEvent(new MouseEvent('mousemove', { clientX: 140, clientY: 300, bubbles: true }));
    window.dispatchEvent(new MouseEvent('mousemove', { clientX: 160, clientY: 300, bubbles: true }));
    window.dispatchEvent(new MouseEvent('mouseup', { clientX: 160, clientY: 300, bubbles: true }));

    expect(resizeSpy).toHaveBeenCalledTimes(1);
    expect(resizeSpy).toHaveBeenCalledWith('sidebar', 320);
  });
});
