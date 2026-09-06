import type { CommandHandler, GraphEditOutcome } from "../types";
import { applyGraphDraftMutation } from "../../graphDraft/graphDraftCoordinator";

export interface DisconnectNodeArgs {
  nodeId: string;
}

export const disconnectNodeCommand: CommandHandler<DisconnectNodeArgs, GraphEditOutcome> = {
  execute(graphPath, args) {
    return applyGraphDraftMutation({
      graphPath,
      mutation: {
        type: "disconnectNode",
        payload: { nodeId: args.nodeId },
      },
    });
  },
};
