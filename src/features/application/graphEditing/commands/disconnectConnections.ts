import type { CommandHandler, GraphEditOutcome } from "../types";
import { applyGraphDraftMutation } from "../../graphDraft/graphDraftCoordinator";

export interface DisconnectConnectionsArgs {
  connectionIds: string[];
}

export const disconnectConnectionsCommand: CommandHandler<
  DisconnectConnectionsArgs,
  GraphEditOutcome
> = {
  execute(graphPath, args) {
    if (args.connectionIds.length === 0) return { status: "unavailable" };
    return applyGraphDraftMutation({
      graphPath,
      mutation: {
        type: "disconnectConnections",
        payload: { connectionIds: args.connectionIds },
      },
    });
  },
};
