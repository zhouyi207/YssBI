import { createReadProjection, useReadProjection } from "@/features/core/state/readProjection";

import type { DeepReadonly } from "@/shared/types/deepReadonly";
import type { SidebarDragState } from "@/features/core/dnd";
import { canvasDropHandlerStore, type CanvasDropHandler } from "./canvasDropHandlerStore";
import { useSidebarDragStore } from "./sidebarDragStore";

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

function buildSnapshot(): DeepReadonly<SidebarDragUiSnapshot> {
  return {
    activeDrag: useSidebarDragStore.getState().activeDrag,
  };
}

const projection = createReadProjection(buildSnapshot, [useSidebarDragStore]);
export const getSidebarDragUiSnapshot = projection.getSnapshot;
export const subscribeSidebarDragUi = projection.subscribe;
export function useSidebarDragUi<T>(
  selector: (snapshot: DeepReadonly<SidebarDragUiSnapshot>) => T,
): T {
  return useReadProjection(projection, selector);
}

export const sidebarDragUi: SidebarDragUiCapability = {
  getSnapshot: getSidebarDragUiSnapshot,
  subscribe: subscribeSidebarDragUi,
  setActiveDrag: (drag) =>
    useSidebarDragStore.getState().setActiveDrag(drag as SidebarDragState | null),
  updatePosition: (x, y) => useSidebarDragStore.getState().updatePosition(x, y),
  setCanvasDropHandler: (panelInstanceId, handler) =>
    canvasDropHandlerStore.setHandler(panelInstanceId, handler),
  getCanvasDropHandler: (panelInstanceId) => canvasDropHandlerStore.getHandler(panelInstanceId),
  subscribeCanvasDropHandlers: (listener) => canvasDropHandlerStore.subscribe(listener),
};
