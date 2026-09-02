import type { CommandHandler, GraphDraftCommandResult } from "../types";
import { executeGraphDraftIntent } from "./executeGraphDraftIntent";

export interface DuplicateSubgraphArgs {
  nodeIds: string[];
  offset: { x: number; y: number };
}

export const duplicateSubgraphCommand: CommandHandler<
  DuplicateSubgraphArgs,
  GraphDraftCommandResult
> = {
  execute(graphPath, args) {
    if (args.nodeIds.length === 0) return false;
    return executeGraphDraftIntent(graphPath, {
      type: "duplicateSubgraph",
      payload: {
        nodeIds: [...args.nodeIds],
        offset: { ...args.offset },
      },
    });
  },
};
