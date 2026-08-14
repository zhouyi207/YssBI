// @vitest-environment happy-dom

import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';
import { ScrollArea } from './scroll-area';

describe('ScrollArea', () => {
  let host: HTMLDivElement;
  let root: Root;

  beforeAll(() => {
    (globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
  });

  afterAll(() => {
    (globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT = false;
  });

  beforeEach(() => {
    host = document.createElement('div');
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
  });

  it('keeps viewport refs and scroll events on the shadcn scroll area viewport', () => {
    const viewportRef = { current: null as HTMLDivElement | null };
    const onScroll = vi.fn();

    act(() => {
      root.render(
        <ScrollArea viewportRef={viewportRef} onViewportScroll={onScroll}>
          <div>Content</div>
        </ScrollArea>,
      );
    });

    expect(host.querySelector('[data-slot="scroll-area"]')).not.toBeNull();
    expect(viewportRef.current).toBe(host.querySelector('[data-slot="scroll-area-viewport"]'));

    act(() => {
      viewportRef.current?.dispatchEvent(new Event('scroll'));
    });

    expect(onScroll).toHaveBeenCalledOnce();
  });
});
