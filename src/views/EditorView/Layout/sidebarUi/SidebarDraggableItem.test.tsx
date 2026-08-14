// @vitest-environment happy-dom
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { DRAG_TYPES, type SidebarDragPayload } from '@/features/core/dnd';

const draggable = vi.hoisted(() => ({
  inputs: [] as Array<{ id: string; data: unknown; disabled: boolean }>,
  onPointerDown: vi.fn(),
}));

vi.mock('@dnd-kit/core', () => ({
  useDraggable: (input: { id: string; data: unknown; disabled: boolean }) => {
    draggable.inputs.push(input);
    return {
      attributes: { 'aria-roledescription': 'draggable' },
      listeners: { onPointerDown: draggable.onPointerDown },
      setNodeRef: vi.fn(),
    };
  },
}));

import { SidebarDraggableItem } from './SidebarDraggableItem';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const dragData = {
  type: DRAG_TYPES.GRAPH_RESOURCE,
  sidebarResource: {
    id: 'functions/Revenue.yssbi-function',
    name: 'Revenue',
    type: 'function',
  },
} satisfies SidebarDragPayload;

describe('SidebarDraggableItem', () => {
  let host: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    vi.clearAllMocks();
    draggable.inputs.length = 0;
    host = document.createElement('div');
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
  });

  it('uses the pointer cursor and registers the exact drag payload', () => {
    act(() => root.render(
      <SidebarDraggableItem id="function-row" dragData={dragData}>
        Revenue
      </SidebarDraggableItem>,
    ));
    const row = host.firstElementChild as HTMLElement;

    expect(row.classList.contains('cursor-pointer')).toBe(true);
    expect(row.classList.contains('cursor-grab')).toBe(false);
    expect(row.classList.contains('active:cursor-grabbing')).toBe(false);
    expect(draggable.inputs[0]).toEqual({
      id: 'sidebar-item-function-row',
      data: dragData,
      disabled: false,
    });
    expect(draggable.inputs[0]?.data).toBe(dragData);
  });

  it('keeps descriptor-unavailable rows disabled and non-draggable', () => {
    act(() => root.render(
      <SidebarDraggableItem
        id="function-row"
        dragData={null}
        dragDisabledReason="Descriptor unavailable"
      >
        Revenue
      </SidebarDraggableItem>,
    ));
    const row = host.firstElementChild as HTMLElement;

    expect(row.getAttribute('aria-disabled')).toBe('true');
    expect(row.classList.contains('cursor-pointer')).toBe(false);
    expect(row.classList.contains('cursor-grab')).toBe(false);
    expect(row.classList.contains('active:cursor-grabbing')).toBe(false);
    expect(row.hasAttribute('aria-roledescription')).toBe(false);

    act(() => row.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true })));

    expect(draggable.onPointerDown).not.toHaveBeenCalled();
    expect(draggable.inputs[0]).toEqual({
      id: 'sidebar-item-function-row',
      data: {},
      disabled: true,
    });
  });
});
