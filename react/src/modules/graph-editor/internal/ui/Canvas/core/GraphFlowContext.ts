import { createContext, useContext } from "react";
import { createStore } from "zustand/vanilla";
import { useStore } from "zustand";
import type { GraphContextMenuActions } from "@/features/application/editor";
import { EMPTY_FLOW_INTERACTION, type FlowInteractionProjection } from "./graphFlowModel";

export interface GraphFlowContextValue {
  graphPath: string;
  groupId: string;
  interactive: boolean;
  contextMenuActions: GraphContextMenuActions | null;
}

export interface GraphFlowInteractionValue extends FlowInteractionProjection {
  targetId: string | null;
  replacedConnectionIds: ReadonlySet<string>;
}

export const GraphFlowContext = createContext<GraphFlowContextValue | null>(null);

export function useGraphFlowContext() {
  const context = useContext(GraphFlowContext);
  if (!context) throw new Error("Graph flow content requires its panel-scoped canvas");
  return context;
}

export function createGraphFlowInteractionStore() {
  return createStore<GraphFlowInteractionValue>(() => ({
    ...EMPTY_FLOW_INTERACTION,
    targetId: null,
    replacedConnectionIds: new Set(),
  }));
}

const emptyInteraction = createGraphFlowInteractionStore();
export const GraphFlowInteractionContext = createContext<ReturnType<
  typeof createGraphFlowInteractionStore
> | null>(null);

export function useGraphFlowInteraction<T>(selector: (state: GraphFlowInteractionValue) => T): T {
  return useStore(useContext(GraphFlowInteractionContext) ?? emptyInteraction, selector);
}
