import { create } from "zustand";
import type { GraphPath } from "@/shared/types";
import type { PinData } from "@/features/domain/editorProjection/graphRuntimeTypes";

export type CanvasGestureType =
  | "panning"
  | "selecting"
  | "draggingNodes"
  | "drawingConnection"
  | "movingConnections";

export interface CanvasGestureScope {
  groupId: string;
  panelInstanceId: string;
}

/** Only cancellation/palette ownership is shared; the renderer owns live pointer geometry. */
export type CanvasInteraction =
  | { type: "idle" }
  | { type: CanvasGestureType; session: CanvasGestureScope }
  | {
      type: "pendingNodeCreation";
      session: CanvasGestureScope & {
        graphPath: GraphPath;
        source: PinData | null;
        screenX: number;
        screenY: number;
      };
    };

export const IDLE_CANVAS_INTERACTION: CanvasInteraction = { type: "idle" };

export function getCanvasInteraction(
  state: Pick<GraphInteractionState, "interactions">,
  graphPath: GraphPath,
  groupId: string,
): CanvasInteraction {
  const interaction = state.interactions[graphPath];
  return interaction?.type !== "idle" && interaction?.session.groupId === groupId
    ? interaction
    : IDLE_CANVAS_INTERACTION;
}

export interface GraphInteractionState {
  interactions: Record<string, CanvasInteraction>;
  startInteraction(
    graphPath: GraphPath,
    interaction: Exclude<CanvasInteraction, { type: "idle" }>,
  ): void;
  finishInteraction(graphPath: GraphPath, groupId: string): CanvasInteraction["type"];
  cancelInteraction(graphPath: GraphPath, groupId: string): CanvasInteraction["type"];
  clearGraphInteraction(graphPath: GraphPath): void;
}

export const useGraphInteractionStore = create<GraphInteractionState>((set, get) => {
  const finishInteraction = (graphPath: GraphPath, groupId: string): CanvasInteraction["type"] => {
    const previous = getCanvasInteraction(get(), graphPath, groupId);
    if (previous.type === "idle") return "idle";
    set((state) => ({
      interactions: { ...state.interactions, [graphPath]: IDLE_CANVAS_INTERACTION },
    }));
    return previous.type;
  };
  return {
    interactions: {},
    startInteraction: (graphPath, interaction) =>
      set((state) => ({
        interactions: { ...state.interactions, [graphPath]: structuredClone(interaction) },
      })),
    finishInteraction,
    cancelInteraction: finishInteraction,
    clearGraphInteraction: (graphPath) =>
      set((state) => {
        const interactions = { ...state.interactions };
        delete interactions[graphPath];
        return { interactions };
      }),
  };
});
