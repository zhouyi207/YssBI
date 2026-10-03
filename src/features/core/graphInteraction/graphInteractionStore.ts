import { create } from "zustand";
import type { GraphPath } from "@/shared/types";
import type { PortAddressDto } from "@/shared/types/domain/editorProjection";

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
        source: PortAddressDto;
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
  ): Exclude<CanvasInteraction, { type: "idle" }>;
  finishInteraction(graphPath: GraphPath, groupId: string): CanvasInteraction["type"];
  cancelInteraction(graphPath: GraphPath, groupId: string): CanvasInteraction["type"];
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
    startInteraction: (graphPath, interaction) => {
      const installed = structuredClone(interaction);
      set((state) => ({
        interactions: { ...state.interactions, [graphPath]: installed },
      }));
      return installed;
    },
    finishInteraction,
    cancelInteraction: finishInteraction,
  };
});
