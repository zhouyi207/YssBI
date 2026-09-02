import type { NodePositionDto } from "@/shared/types/domain/editorProjection";
import type { CommandHandler, GraphDraftCommandResult } from "../types";
import { executeGraphDraftIntent } from "./executeGraphDraftIntent";

export interface MoveNodesArgs {
  positions: Array<{ nodeId: string; position: NodePositionDto }>;
}

export const moveNodesCommand: CommandHandler<MoveNodesArgs, GraphDraftCommandResult> = {
  execute(graphPath, args) {
    return executeGraphDraftIntent(graphPath, {
      type: "moveNodes",
      payload: { positions: args.positions },
    });
  },
};
