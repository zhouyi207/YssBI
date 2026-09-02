import type { CommandHandler, GraphDraftCommandResult } from "../types";
import { executeGraphDraftIntent } from "./executeGraphDraftIntent";

export interface InsertRerouteArgs {
  connectionId: string;
  position: { x: number; y: number };
}

export const insertRerouteCommand: CommandHandler<InsertRerouteArgs, GraphDraftCommandResult> = {
  execute(graphPath, args) {
    return executeGraphDraftIntent(graphPath, {
      type: "insertReroute",
      payload: args,
    });
  },
};
