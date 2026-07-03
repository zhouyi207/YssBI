import { useGestureStore } from '@/features/core/gesture';
import type { EditorGesture } from '@/shared/types/ui';

export type DragPreviewState = {
  active: boolean;
  dragDelta: { x: number; y: number };
  dragNodeIds: ReadonlySet<string>;
  groupId?: string;
};

const IDLE: DragPreviewState = {
  active: false,
  dragDelta: { x: 0, y: 0 },
  dragNodeIds: new Set<string>(),
};

let previewState: DragPreviewState = IDLE;
const listeners = new Set<() => void>();

function setsEqual(a: ReadonlySet<string>, b: ReadonlySet<string>): boolean {
  if (a.size !== b.size) return false;
  for (const id of a) {
    if (!b.has(id)) return false;
  }
  return true;
}

function gestureToPreview(gesture: EditorGesture): DragPreviewState {
  if (gesture?.type !== 'drag') return IDLE;
  return {
    active: true,
    dragDelta: gesture.dragDelta ?? { x: 0, y: 0 },
    dragNodeIds: new Set(gesture.dragNodeIds ?? []),
    groupId: gesture.groupId,
  };
}

function publish(next: DragPreviewState): void {
  if (
    previewState.active === next.active
    && previewState.dragDelta.x === next.dragDelta.x
    && previewState.dragDelta.y === next.dragDelta.y
    && previewState.groupId === next.groupId
    && setsEqual(previewState.dragNodeIds, next.dragNodeIds)
  ) {
    return;
  }
  previewState = next;
  listeners.forEach((listener) => listener());
}

export function getDragPreview(): DragPreviewState {
  return previewState;
}

export function subscribeDragPreview(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

/** Sync drag preview from gesture store; mount once per canvas shell. */
export function bindDragPreviewToGestureStore(): () => void {
  publish(gestureToPreview(useGestureStore.getState().gesture));
  return useGestureStore.subscribe((state) => {
    publish(gestureToPreview(state.gesture));
  });
}
