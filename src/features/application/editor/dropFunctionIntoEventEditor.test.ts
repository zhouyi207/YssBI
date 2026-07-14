import { beforeEach, describe, expect, it } from 'vitest';
import { DRAG_TYPES } from '@/features/core/dnd';
import { useSidebarDragStore } from '@/features/core/sidebarDrag';
import {
  resolveDropIntoEditorDragState,
  resolveDropPointerFromDragEnd,
} from './dropFunctionIntoEventEditor';

describe('dropFunctionIntoEventEditor', () => {
  beforeEach(() => {
    useSidebarDragStore.getState().setActiveDrag(null);
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
});
