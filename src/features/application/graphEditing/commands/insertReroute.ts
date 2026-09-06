import type { CommandHandler, GraphEditOutcome } from "../types";
import { applyGraphDraftMutation } from "../../graphDraft/graphDraftCoordinator";

export interface InsertRerouteArgs {
  connectionId: string;
  position: { x: number; y: number };
}

export const insertRerouteCommand: CommandHandler<InsertRerouteArgs, GraphEditOutcome> = {
  execute(graphPath, args) {
    return applyGraphDraftMutation({
      graphPath,
      mutation: {
        type: "insertReroute",
        payload: args,
      },
    });
  },
};
