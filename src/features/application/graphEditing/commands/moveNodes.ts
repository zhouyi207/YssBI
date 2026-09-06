import type { NodePositionDto } from "@/shared/types/domain/editorProjection";
import type { CommandHandler, GraphEditOutcome } from "../types";
import { applyGraphDraftMutation } from "../../graphDraft/graphDraftCoordinator";

export interface MoveNodesArgs {
  positions: Array<{ nodeId: string; position: NodePositionDto }>;
}

export const moveNodesCommand: CommandHandler<MoveNodesArgs, GraphEditOutcome> = {
  execute(graphPath, args) {
    return applyGraphDraftMutation({
      graphPath,
      mutation: {
        type: "moveNodes",
        payload: { positions: args.positions },
      },
    });
  },
};
