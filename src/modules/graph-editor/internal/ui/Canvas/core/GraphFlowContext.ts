import { createContext, useContext } from "react";
import type { GraphContextMenuActions } from "@/features/application/editor";
import type { PinData } from "@/features/domain/editorProjection/graphRuntimeTypes";
import type { GraphFlowModel, FlowConnectionFeedback } from "./graphFlowModel";

export interface GraphFlowContextValue {
  graphPath: string;
  groupId: string;
  interactive: boolean;
  contextMenuActions: GraphContextMenuActions | null;
  model: GraphFlowModel;
  sourcePin: PinData | null;
  feedbackForPin(pinId: string): FlowConnectionFeedback | null;
}

export const GraphFlowContext = createContext<GraphFlowContextValue | null>(null);

export function useGraphFlowContext() {
  const context = useContext(GraphFlowContext);
  if (!context) throw new Error("Graph flow content requires its panel-scoped canvas");
  return context;
}
