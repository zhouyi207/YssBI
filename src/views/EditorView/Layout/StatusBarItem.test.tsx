// @vitest-environment happy-dom
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { StatusBarItem } from './StatusBarItem';
import type { StatusBarItemViewModel } from '@/features/core/statusBar';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

function interactiveItem(overrides: Partial<StatusBarItemViewModel> = {}): StatusBarItemViewModel {
  return {
    id: 'execution-status',
    alignment: 'right',
    priority: 40,
    content: 'Idle',
    ariaLabel: 'Open logs panel',
    tooltip: 'Open logs panel',
    onClick: vi.fn(),
    ...overrides,
  };
}

describe('StatusBarItem', () => {
  let host: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    host = document.createElement('div');
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    host.remove();
  });

  it('exposes an accessible name and keyboard activation for interactive items', () => {
    const item = interactiveItem({ tooltip: undefined });

    act(() => {
      root.render(<StatusBarItem item={item} />);
    });

    const button = host.querySelector('[role="button"]') as HTMLElement;
    expect(button).toBeTruthy();
    expect(button.getAttribute('aria-label')).toBe('Open logs panel');
    expect(button.getAttribute('tabindex')).toBe('0');

    act(() => {
      button.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
    });
    expect(item.onClick).toHaveBeenCalledTimes(1);
  });

  it('does not render a button role for read-only items', () => {
    act(() => {
      root.render(
        <StatusBarItem
          item={{
            id: 'node-count',
            alignment: 'right',
            priority: 10,
            content: '5 Nodes',
          }}
        />,
      );
    });

    expect(host.querySelector('[role="button"]')).toBeNull();
    expect(host.textContent).toContain('5 Nodes');
  });
});
