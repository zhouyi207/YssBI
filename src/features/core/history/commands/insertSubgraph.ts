import type { CommandHandler, GraphDraftCommandResult } from "../types";
import { executeGraphDraftIntent } from "./executeGraphDraftIntent";

export interface InsertSubgraphArgs {
  snapshotJson: string;
  anchor: { x: number; y: number };
}

export const insertSubgraphCommand: CommandHandler<InsertSubgraphArgs, GraphDraftCommandResult> = {
  execute(graphPath, args) {
    return executeGraphDraftIntent(graphPath, {
      type: "insertSubgraph",
      payload: {
        snapshotJson: args.snapshotJson,
        anchor: { ...args.anchor },
      },
    });
  },
};
