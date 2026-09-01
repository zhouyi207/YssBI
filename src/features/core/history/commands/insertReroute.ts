import type { CommandHandler, GraphMutationCommandResult } from "../types";
import { executeGraphIntent } from "./executeGraphIntent";

export interface InsertRerouteArgs {
  connectionId: string;
  position: { x: number; y: number };
}

export const insertRerouteCommand: CommandHandler<InsertRerouteArgs, GraphMutationCommandResult> = {
  execute(graphPath, args) {
    return executeGraphIntent(graphPath, {
      type: "insertReroute",
      payload: args,
    });
  },
};
