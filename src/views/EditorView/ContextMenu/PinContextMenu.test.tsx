// @vitest-environment happy-dom

import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { PinContextMenu } from './PinContextMenu';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

let container: HTMLDivElement;
let portal: HTMLDivElement;
let root: Root;

beforeEach(() => {
  container = document.createElement('div');
  portal = document.createElement('div');
  portal.id = 'portal';
  document.body.append(container, portal);
  root = createRoot(container);
});

afterEach(() => {
  act(() => root.unmount());
  container.remove();
  portal.remove();
});

describe('PinContextMenu', () => {
  it('omits promote to variable and retains supported actions', () => {
    renderMenu({ removable: true, hasLinks: true, canReset: true, showView: true, viewEnabled: true });

    expect(item('promoteToVar')).toBeUndefined();
    expect(item('breakLinks')).toBeDefined();
    expect(item('resetValue')).toBeDefined();
    expect(item('view')).toBeDefined();
    expect(item('removePin')).toBeDefined();
  });

  it('preserves disabled state for unavailable supported actions', () => {
    renderMenu({ removable: false, hasLinks: false, canReset: false });

    expect(item('breakLinks')?.disabled).toBe(true);
    expect(item('resetValue')?.disabled).toBe(true);
    expect(item('removePin')?.disabled).toBe(true);
  });

  it('invokes enabled break, reset, view, and remove actions', () => {
    const onBreakLinks = vi.fn();
    const onResetValue = vi.fn();
    const onView = vi.fn();
    const onRemove = vi.fn();
    renderMenu({
      removable: true,
      hasLinks: true,
      canReset: true,
      onBreakLinks,
      onResetValue,
      showView: true,
      viewEnabled: true,
      onView,
      onRemove,
    });

    for (const [label, callback] of [
      ['breakLinks', onBreakLinks],
      ['resetValue', onResetValue],
      ['view', onView],
      ['removePin', onRemove],
    ] as const) {
      act(() => item(label)?.dispatchEvent(new MouseEvent('mousedown', { bubbles: true, button: 0 })));
      expect(callback).toHaveBeenCalledOnce();
    }
  });
});

function item(label: string): HTMLButtonElement | undefined {
  return [...portal.querySelectorAll<HTMLButtonElement>('[role="menuitem"]')]
    .find((button) => button.textContent?.includes(`contextMenu.pin.${label}`));
}

function renderMenu(overrides: Partial<React.ComponentProps<typeof PinContextMenu>> = {}): void {
  act(() => {
    root.render(
      <PinContextMenu
        position={{ x: 0, y: 0 }}
        onClose={vi.fn()}
        {...overrides}
      />,
    );
  });
}
