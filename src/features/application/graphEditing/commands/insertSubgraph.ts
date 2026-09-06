import type { CommandHandler, GraphEditOutcome } from "../types";
import { applyGraphDraftMutation } from "../../graphDraft/graphDraftCoordinator";

export interface InsertSubgraphArgs {
  snapshotJson: string;
  anchor: { x: number; y: number };
}

export const insertSubgraphCommand: CommandHandler<InsertSubgraphArgs, GraphEditOutcome> = {
  execute(graphPath, args) {
    return applyGraphDraftMutation({
      graphPath,
      mutation: {
        type: "insertSubgraph",
        payload: {
          snapshotJson: args.snapshotJson,
          anchor: { ...args.anchor },
        },
      },
    });
  },
};
