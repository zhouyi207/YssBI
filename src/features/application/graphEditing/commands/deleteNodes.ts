import type { CommandHandler, GraphEditOutcome } from "../types";
import { applyGraphDraftMutation } from "../../graphDraft/graphDraftCoordinator";

export interface DeleteNodesArgs {
  nodeIds: string[];
}

export const deleteNodesCommand: CommandHandler<DeleteNodesArgs, GraphEditOutcome> = {
  execute(graphPath, args) {
    if (args.nodeIds.length === 0) return { status: "unavailable" };
    return applyGraphDraftMutation({
      graphPath,
      mutation: {
        type: "deleteNodes",
        payload: { nodeIds: args.nodeIds },
      },
    });
  },
};
