import type { CommandHandler, GraphEditOutcome } from "../types";
import { applyGraphMutation } from "../../graphEditing/graphEditCoordinator";

export interface InsertSubgraphArgs {
  snapshotJson: string;
  anchor: { x: number; y: number };
}

export const insertSubgraphCommand: CommandHandler<InsertSubgraphArgs, GraphEditOutcome> = {
  execute(graphPath, args) {
    return applyGraphMutation({
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
