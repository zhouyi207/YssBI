import type { CommandHandler, GraphMutationCommandResult } from "../types";
import { executeGraphIntent } from "./executeGraphIntent";

export interface DisconnectConnectionsArgs {
  connectionIds: string[];
}

export const disconnectConnectionsCommand: CommandHandler<
  DisconnectConnectionsArgs,
  GraphMutationCommandResult
> = {
  execute(graphPath, args) {
    if (args.connectionIds.length === 0) return false;
    return executeGraphIntent(graphPath, {
      type: "disconnectConnections",
      payload: { connectionIds: args.connectionIds },
    });
  },
};
