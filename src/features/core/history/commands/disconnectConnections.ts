import type { CommandHandler, GraphDraftCommandResult } from "../types";
import { executeGraphDraftIntent } from "./executeGraphDraftIntent";

export interface DisconnectConnectionsArgs {
  connectionIds: string[];
}

export const disconnectConnectionsCommand: CommandHandler<
  DisconnectConnectionsArgs,
  GraphDraftCommandResult
> = {
  execute(graphPath, args) {
    if (args.connectionIds.length === 0) return false;
    return executeGraphDraftIntent(graphPath, {
      type: "disconnectConnections",
      payload: { connectionIds: args.connectionIds },
    });
  },
};
