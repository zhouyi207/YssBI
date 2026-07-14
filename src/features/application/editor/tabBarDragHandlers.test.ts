import { describe, expect, it, vi } from 'vitest';
import { bindTabDragPointerDown } from './tabBarDragHandlers';

function mockElement(matchesSelector: string | null): HTMLElement {
  return {
    closest: (selector: string) => (selector === matchesSelector ? mockElement(matchesSelector) : null),
  } as unknown as HTMLElement;
}

describe('bindTabDragPointerDown', () => {
  it('calls activate and dnd-kit listener on tab pointer down', () => {
    const activate = vi.fn();
    const dragListener = vi.fn();
    const handler = bindTabDragPointerDown({ onPointerDown: dragListener }, activate);
    const tab = mockElement(null);

    handler({ button: 0, target: tab, currentTarget: tab } as unknown as React.PointerEvent);
    expect(activate).toHaveBeenCalledOnce();
    expect(dragListener).toHaveBeenCalledOnce();
  });

  it('skips drag listener on close button', () => {
    const activate = vi.fn();
    const dragListener = vi.fn();
    const handler = bindTabDragPointerDown({ onPointerDown: dragListener }, activate);
    const button = mockElement('button');

    handler({
      button: 0,
      target: button,
      currentTarget: button,
      stopPropagation: vi.fn(),
    } as unknown as React.PointerEvent);
    expect(activate).not.toHaveBeenCalled();
    expect(dragListener).not.toHaveBeenCalled();
  });
});
