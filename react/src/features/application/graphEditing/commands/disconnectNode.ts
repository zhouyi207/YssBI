import type { CommandHandler, GraphEditOutcome } from "../types";
import { applyGraphMutation } from "../../graphEditing/graphEditCoordinator";

export interface DisconnectNodeArgs {
  nodeId: string;
}

export const disconnectNodeCommand: CommandHandler<DisconnectNodeArgs, GraphEditOutcome> = {
  execute(graphPath, args) {
    return applyGraphMutation({
      graphPath,
      mutation: {
        type: "disconnectNode",
        payload: { nodeId: args.nodeId },
      },
    });
  },
};
