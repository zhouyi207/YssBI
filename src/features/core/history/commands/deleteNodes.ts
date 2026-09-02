import type { CommandHandler, GraphDraftCommandResult } from "../types";
import { executeGraphDraftIntent } from "./executeGraphDraftIntent";

export interface DeleteNodesArgs {
  nodeIds: string[];
}

export const deleteNodesCommand: CommandHandler<DeleteNodesArgs, GraphDraftCommandResult> = {
  execute(graphPath, args) {
    if (args.nodeIds.length === 0) return false;
    return executeGraphDraftIntent(graphPath, {
      type: "deleteNodes",
      payload: { nodeIds: args.nodeIds },
    });
  },
};
