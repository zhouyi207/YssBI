import type { CommandHandler, GraphEditOutcome } from "../types";
import { applyGraphMutation } from "../../graphEditing/graphEditCoordinator";

export interface DeleteNodesArgs {
  nodeIds: string[];
}

export const deleteNodesCommand: CommandHandler<DeleteNodesArgs, GraphEditOutcome> = {
  execute(graphPath, args) {
    if (args.nodeIds.length === 0) return { status: "unavailable" };
    return applyGraphMutation({
      graphPath,
      mutation: {
        type: "deleteNodes",
        payload: { nodeIds: args.nodeIds },
      },
    });
  },
};
