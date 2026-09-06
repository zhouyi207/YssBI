import type { CommandHandler, GraphEditOutcome } from "../types";
import { applyGraphDraftMutation } from "../../graphDraft/graphDraftCoordinator";

export interface DuplicateSubgraphArgs {
  nodeIds: string[];
  offset: { x: number; y: number };
}

export const duplicateSubgraphCommand: CommandHandler<DuplicateSubgraphArgs, GraphEditOutcome> = {
  execute(graphPath, args) {
    if (args.nodeIds.length === 0) return { status: "unavailable" };
    return applyGraphDraftMutation({
      graphPath,
      mutation: {
        type: "duplicateSubgraph",
        payload: {
          nodeIds: [...args.nodeIds],
          offset: { ...args.offset },
        },
      },
    });
  },
};
