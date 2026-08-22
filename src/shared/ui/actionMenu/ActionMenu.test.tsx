// @vitest-environment happy-dom
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ActionMenu } from './ActionMenu';

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

describe('ActionMenu', () => {
  let host: HTMLDivElement;
  let portal: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    host = document.createElement('div');
    portal = document.createElement('div');
    portal.id = 'portal';
    document.body.append(host, portal);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
    portal.remove();
  });

  it('bridges a coordinate-open menu to shadcn items and closes after selection', () => {
    const onClick = vi.fn();
    const onClose = vi.fn();
    act(() => root.render(
      <ActionMenu
        position={{ x: 24, y: 36 }}
        sections={[{ items: [{ id: 'open', label: 'Open', onClick }] }]}
        onClose={onClose}
      />,
    ));

    const item = portal.querySelector<HTMLElement>('[role="menuitem"]');
    expect(item?.textContent).toContain('Open');
    act(() => item?.click());

    expect(onClick).toHaveBeenCalledOnce();
    expect(onClose).toHaveBeenCalled();
  });

  it('dismisses on outside pointerdown and Escape without dismissing from menu content', () => {
    const onClose = vi.fn();
    act(() => root.render(
      <ActionMenu
        position={{ x: 24, y: 36 }}
        sections={[{ items: [{ id: 'open', label: 'Open' }] }]}
        onClose={onClose}
      />,
    ));

    const item = portal.querySelector<HTMLElement>('[role="menuitem"]')!;
    act(() => item.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true })));
    expect(onClose).not.toHaveBeenCalled();

    act(() => document.body.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true })));
    act(() => document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true })));
    expect(onClose).toHaveBeenCalled();
  });

  it('does not bubble menu pointerdown into the owner gesture handler', () => {
    const ownerPointerDown = vi.fn();
    act(() => root.render(
      <div onPointerDown={ownerPointerDown}>
        <ActionMenu
          position={{ x: 24, y: 36 }}
          sections={[{ items: [{ id: 'open', label: 'Open' }] }]}
          onClose={vi.fn()}
        />
      </div>,
    ));

    const item = portal.querySelector<HTMLElement>('[role="menuitem"]')!;
    act(() => item.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true, button: 0 })));

    expect(ownerPointerDown).not.toHaveBeenCalled();
  });
});
