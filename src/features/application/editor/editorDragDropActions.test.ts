import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { DragEndEvent } from '@dnd-kit/core';
import { DRAG_TYPES } from '@/features/core/dnd';
import { useSidebarDragStore } from '@/features/core/sidebarDrag';

const mocks = vi.hoisted(() => ({
  clearEditorDragSession: vi.fn(),
  handleGraphResourceDrop: vi.fn(),
  isSidebarSpawnDropAllowed: vi.fn(() => true),
  findSidebarDropCanvasAtPointer: vi.fn(() => ({
    groupId: 'group-a',
    bounds: {} as DOMRect,
  })),
  resolveDropIntoEditorDragState: vi.fn(),
  resolveDropPointerFromDragEnd: vi.fn(() => ({ x: 240, y: 180 })),
  tryDropFunctionIntoCanvas: vi.fn(async () => true),
  activateEditorGroup: vi.fn(),
}));

vi.mock('./useEditorDragPreviewMonitor', () => ({
  clearEditorDragSession: mocks.clearEditorDragSession,
}));
vi.mock('./handleGraphResourceDrop', () => ({
  handleGraphResourceDrop: mocks.handleGraphResourceDrop,
}));
vi.mock('./sidebarSpawnDropPolicy', () => ({
  isSidebarSpawnDropAllowed: mocks.isSidebarSpawnDropAllowed,
  findSidebarDropCanvasAtPointer: mocks.findSidebarDropCanvasAtPointer,
}));
vi.mock('./dropFunctionIntoEventEditor', () => ({
  resolveDropIntoEditorDragState: mocks.resolveDropIntoEditorDragState,
  resolveDropPointerFromDragEnd: mocks.resolveDropPointerFromDragEnd,
  tryDropFunctionIntoCanvas: mocks.tryDropFunctionIntoCanvas,
}));
vi.mock('./switchEditorTab', () => ({
  activateEditorGroup: mocks.activateEditorGroup,
}));

import { executeEditorDragEnd } from './editorDragDropActions';

describe('executeEditorDragEnd', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useSidebarDragStore.getState().setActiveDrag({
      type: DRAG_TYPES.GRAPH_RESOURCE,
      sidebarResource: {
        id: 'functions/Helper.yssbi-function',
        name: 'Helper',
        type: 'function',
      },
      x: 10,
      y: 20,
    });
    const activeDrag = useSidebarDragStore.getState().activeDrag;
    mocks.resolveDropIntoEditorDragState.mockReturnValue(
      activeDrag ? { ...activeDrag, x: 240, y: 180 } : null,
    );
  });

  it('routes an unmodified function drop to node creation even without droppable metadata', async () => {
    const payload = {
      type: DRAG_TYPES.GRAPH_RESOURCE,
      sidebarResource: {
        id: 'functions/Helper.yssbi-function',
        name: 'Helper',
        type: 'function' as const,
      },
    };
    const event = {
      active: { data: { current: payload } },
      over: null,
      activatorEvent: { clientX: 100, clientY: 100 },
      delta: { x: 140, y: 80 },
    } as unknown as DragEndEvent;

    await executeEditorDragEnd(event, { finishSidebarDrag: vi.fn() });

    expect(mocks.findSidebarDropCanvasAtPointer).toHaveBeenCalledWith(240, 180);
    expect(mocks.tryDropFunctionIntoCanvas).toHaveBeenCalledWith(
      'group-a',
      expect.objectContaining({ type: DRAG_TYPES.GRAPH_RESOURCE, x: 240, y: 180 }),
      { altKey: false, ctrlKey: false, shiftKey: false },
    );
    expect(mocks.handleGraphResourceDrop).not.toHaveBeenCalled();
  });
});
