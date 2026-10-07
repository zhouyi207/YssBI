import type { CommandHandler, GraphEditOutcome } from "../types";
import { applyGraphMutation } from "../../graphEditing/graphEditCoordinator";

export interface InsertRerouteArgs {
  connectionId: string;
  position: { x: number; y: number };
}

export const insertRerouteCommand: CommandHandler<InsertRerouteArgs, GraphEditOutcome> = {
  execute(graphPath, args) {
    return applyGraphMutation({
      graphPath,
      mutation: {
        type: "insertReroute",
        payload: args,
      },
    });
  },
};
