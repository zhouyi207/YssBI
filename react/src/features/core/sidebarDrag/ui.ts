import { createReadProjection, useReadProjection } from "@/features/core/state/readProjection";

import type { DeepReadonly } from "@/shared/types/deepReadonly";
import type { SidebarDragState } from "@/features/core/dnd";
import { useSidebarDragStore } from "./sidebarDragStore";

export interface SidebarDragUiSnapshot {
  readonly activeDrag: DeepReadonly<SidebarDragState> | null;
}

export interface SidebarDragUiCapability {
  readonly setActiveDrag: (drag: DeepReadonly<SidebarDragState> | null) => void;
  readonly updatePosition: (x: number, y: number) => void;
}

function buildSnapshot(): DeepReadonly<SidebarDragUiSnapshot> {
  return {
    activeDrag: useSidebarDragStore.getState().activeDrag,
  };
}

const projection = createReadProjection(buildSnapshot, [useSidebarDragStore]);
export function useSidebarDragUi<T>(
  selector: (snapshot: DeepReadonly<SidebarDragUiSnapshot>) => T,
): T {
  return useReadProjection(projection, selector);
}

export const sidebarDragUi: SidebarDragUiCapability = {
  setActiveDrag: (drag) =>
    useSidebarDragStore.getState().setActiveDrag(drag as SidebarDragState | null),
  updatePosition: (x, y) => useSidebarDragStore.getState().updatePosition(x, y),
};
