import { beforeEach, describe, expect, it, vi } from 'vitest';
import { DRAG_TYPES } from '@/features/core/dnd';
import { canvasDropHandlerStore, useSidebarDragStore } from '@/features/core/sidebarDrag';

vi.mock('@/features/application/editor/canvasDrop', () => ({
  canCreateFunctionNodeInGraph: vi.fn(() => true),
}));
vi.mock('@/features/application/editor/switchEditorTab', () => ({
  activateEditorGroup: vi.fn(),
}));
import {
  resolveDropIntoEditorDragState,
  resolveDropPointerFromDragEnd,
  tryDropFunctionIntoCanvas,
} from './dropFunctionIntoEventEditor';

describe('dropFunctionIntoEventEditor', () => {
  beforeEach(() => {
    useSidebarDragStore.getState().setActiveDrag(null);
    canvasDropHandlerStore.setHandler('group-a', null);
  });

  it('resolveDropPointerFromDragEnd uses activator position plus delta', () => {
    const pointer = resolveDropPointerFromDragEnd({
      activatorEvent: { clientX: 100, clientY: 200 } as MouseEvent,
      delta: { x: 40, y: -10 },
    });
    expect(pointer).toEqual({ x: 140, y: 190 });
  });

  it('resolveDropIntoEditorDragState preserves captured drag coords after sidebar drag clears', () => {
    useSidebarDragStore.getState().setActiveDrag({
      type: DRAG_TYPES.GRAPH_RESOURCE,
      sidebarResource: {
        id: 'functions/A.yssbi-function',
        name: 'A',
        type: 'function',
      },
      x: 80,
      y: 120,
    });

    const captured = useSidebarDragStore.getState().activeDrag;
    useSidebarDragStore.getState().setActiveDrag(null);

    const dropState = resolveDropIntoEditorDragState(
      { id: 'functions/A.yssbi-function', name: 'A', type: 'function' },
      { x: 300, y: 400 },
      captured,
    );

    expect(dropState).toEqual({
      type: DRAG_TYPES.GRAPH_RESOURCE,
      sidebarResource: {
        id: 'functions/A.yssbi-function',
        name: 'A',
        type: 'function',
      },
      x: 300,
      y: 400,
    });
  });

  it('routes a function resource to the canvas handler without requiring Shift', async () => {
    const handler = vi.fn(async () => true);
    canvasDropHandlerStore.setHandler('group-a', handler);

    const dragState = {
      type: DRAG_TYPES.GRAPH_RESOURCE,
      sidebarResource: {
        id: 'functions/Helper.yssbi-function',
        name: 'Helper',
        type: 'function' as const,
      },
      x: 240,
      y: 180,
    };

    await expect(tryDropFunctionIntoCanvas('group-a', dragState, {
      shiftKey: false,
      altKey: false,
      ctrlKey: false,
    })).resolves.toBe(true);

    expect(handler).toHaveBeenCalledWith(dragState, {
      shiftKey: false,
      altKey: false,
      ctrlKey: false,
    });
  });
});
