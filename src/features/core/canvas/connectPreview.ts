import { useGestureStore } from '@/features/core/gesture';
import type { Pin } from '@/shared/types/domain';
import type { EditorGesture } from '@/shared/types/ui';

export type ConnectPreviewState = {
  active: boolean;
  startPin: Pin | null;
  worldX: number;
  worldY: number;
  groupId?: string;
};

const IDLE: ConnectPreviewState = {
  active: false,
  startPin: null,
  worldX: 0,
  worldY: 0,
};

let previewState: ConnectPreviewState = IDLE;
const listeners = new Set<() => void>();

function gestureToPreview(gesture: EditorGesture): ConnectPreviewState {
  if (gesture?.type !== 'connect') return IDLE;
  return {
    active: true,
    startPin: gesture.startPin,
    worldX: gesture.worldX ?? 0,
    worldY: gesture.worldY ?? 0,
    groupId: gesture.groupId,
  };
}

function publish(next: ConnectPreviewState): void {
  if (
    previewState.active === next.active
    && previewState.startPin === next.startPin
    && previewState.worldX === next.worldX
    && previewState.worldY === next.worldY
    && previewState.groupId === next.groupId
  ) {
    return;
  }
  previewState = next;
  listeners.forEach((listener) => listener());
}

export function getConnectPreview(): ConnectPreviewState {
  return previewState;
}

export function subscribeConnectPreview(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

/** Sync connect preview from gesture store; mount once per canvas shell. */
export function bindConnectPreviewToGestureStore(): () => void {
  publish(gestureToPreview(useGestureStore.getState().gesture));
  return useGestureStore.subscribe((state) => {
    publish(gestureToPreview(state.gesture));
  });
}
