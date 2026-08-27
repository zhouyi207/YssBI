import { useSyncExternalStore } from 'react';

import type { DeepReadonly } from '@/features/core/projection/deepReadonly';
import type { SidebarDragState } from '@/features/core/dnd';
import {
  canvasDropHandlerStore,
  type CanvasDropHandler,
} from './canvasDropHandlerStore';
import { useSidebarDragStore } from './sidebarDragStore';

export interface SidebarDragUiSnapshot {
  readonly activeDrag: DeepReadonly<SidebarDragState> | null;
}

export interface SidebarDragUiCapability {
  readonly getSnapshot: () => DeepReadonly<SidebarDragUiSnapshot>;
  readonly subscribe: (listener: () => void) => () => void;
  readonly setActiveDrag: (drag: DeepReadonly<SidebarDragState> | null) => void;
  readonly updatePosition: (x: number, y: number) => void;
  readonly setCanvasDropHandler: (
    panelInstanceId: string,
    handler: CanvasDropHandler | null,
  ) => void;
  readonly getCanvasDropHandler: (panelInstanceId: string) => CanvasDropHandler | null;
  readonly subscribeCanvasDropHandlers: (listener: () => void) => () => void;
}

function cloneAndFreeze<T>(value: T): T {
  if (Array.isArray(value)) return Object.freeze(value.map(cloneAndFreeze)) as T;
  if (value === null || typeof value !== 'object') return value;
  return Object.freeze(Object.fromEntries(
    Object.entries(value as Record<string, unknown>)
      .map(([key, nested]) => [key, cloneAndFreeze(nested)]),
  )) as T;
}

function buildSnapshot(): DeepReadonly<SidebarDragUiSnapshot> {
  return Object.freeze({
    activeDrag: cloneAndFreeze(useSidebarDragStore.getState().activeDrag),
  });
}

let currentSnapshot = buildSnapshot();
const listeners = new Set<() => void>();

function refreshSnapshot(): void {
  currentSnapshot = buildSnapshot();
  for (const listener of listeners) listener();
}

useSidebarDragStore.subscribe(refreshSnapshot);

export function getSidebarDragUiSnapshot(): DeepReadonly<SidebarDragUiSnapshot> {
  return currentSnapshot;
}

export function subscribeSidebarDragUi(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function useSidebarDragUi<T>(
  selector: (snapshot: DeepReadonly<SidebarDragUiSnapshot>) => T,
): T {
  const snapshot = useSyncExternalStore(
    subscribeSidebarDragUi,
    getSidebarDragUiSnapshot,
    getSidebarDragUiSnapshot,
  );
  return selector(snapshot);
}

export const sidebarDragUi: SidebarDragUiCapability = {
  getSnapshot: getSidebarDragUiSnapshot,
  subscribe: subscribeSidebarDragUi,
  setActiveDrag: (drag) =>
    useSidebarDragStore.getState().setActiveDrag(drag as SidebarDragState | null),
  updatePosition: (x, y) => useSidebarDragStore.getState().updatePosition(x, y),
  setCanvasDropHandler: (panelInstanceId, handler) =>
    canvasDropHandlerStore.setHandler(panelInstanceId, handler),
  getCanvasDropHandler: (panelInstanceId) =>
    canvasDropHandlerStore.getHandler(panelInstanceId),
  subscribeCanvasDropHandlers: (listener) =>
    canvasDropHandlerStore.subscribe(listener),
};
