// @vitest-environment happy-dom
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { TooltipProvider } from '@/components/ui/tooltip';
import { SidebarEmptyState } from './SidebarEmptyState';
import { SidebarSectionEmptyState } from './SidebarSectionEmptyState';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

describe('Sidebar empty-state components', () => {
  let host: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    host = document.createElement('div');
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
  });

  it('renders a wrapping tab-level state without a scrollbar viewport', () => {
    act(() => {
      root.render(
        <SidebarEmptyState
          title="Node catalog unavailable"
          description="Waiting for stable catalog descriptors"
        />,
      );
    });

    expect(host.textContent).toContain('Node catalog unavailable');
    expect(host.textContent).toContain('Waiting for stable catalog descriptors');
    expect(host.querySelector('.overlay-scrollbar-viewport')).toBeNull();
    expect(host.firstElementChild?.className).toContain('px-3');
  });

  it('renders a compact truncated section state with the full accessible label', () => {
    act(() => {
      root.render(
        <TooltipProvider>
          <SidebarSectionEmptyState
            level={1}
            message="A deliberately long section empty-state message"
          />
        </TooltipProvider>,
      );
    });

    const message = host.querySelector(
      '[aria-label="A deliberately long section empty-state message"]',
    );
    expect(message).toBeInstanceOf(HTMLElement);
    expect(message?.className).toContain('truncate');
    expect(message?.closest('.h-7')).not.toBeNull();
    expect((message as HTMLElement).tabIndex).toBe(0);

    act(() => (message as HTMLElement).focus());
    expect(document.activeElement).toBe(message);
  });

  it('forwards section context-menu events', () => {
    let contextMenuCalls = 0;
    act(() => {
      root.render(
        <TooltipProvider>
          <SidebarSectionEmptyState
            level={1}
            message="No events"
            onContextMenu={() => {
              contextMenuCalls += 1;
            }}
          />
        </TooltipProvider>,
      );
    });

    const message = host.querySelector('[aria-label="No events"]');
    act(() => {
      message?.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true }));
    });

    expect(contextMenuCalls).toBe(1);
  });
});
