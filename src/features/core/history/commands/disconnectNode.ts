import type { CommandHandler, GraphDraftCommandResult } from "../types";
import { executeGraphDraftIntent } from "./executeGraphDraftIntent";

export interface DisconnectNodeArgs {
  nodeId: string;
}

export const disconnectNodeCommand: CommandHandler<DisconnectNodeArgs, GraphDraftCommandResult> = {
  execute(graphPath, args) {
    return executeGraphDraftIntent(graphPath, {
      type: "disconnectNode",
      payload: { nodeId: args.nodeId },
    });
  },
};
