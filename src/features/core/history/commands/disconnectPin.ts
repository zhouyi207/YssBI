import { useGraphProjectionStore } from "@/features/core/dataStore/graphProjectionStore";
import type { CommandHandler, GraphDraftCommandResult } from "../types";
import { executeGraphDraftIntent } from "./executeGraphDraftIntent";

export interface DisconnectPortArgs {
  pinId: string;
}

export const disconnectPortCommand: CommandHandler<DisconnectPortArgs, GraphDraftCommandResult> = {
  execute(graphPath, args) {
    const pin = useGraphProjectionStore.getState().getGraphPin(graphPath, args.pinId);
    if (!pin) return false;
    return executeGraphDraftIntent(graphPath, {
      type: "disconnectPort",
      payload: { address: pin.address },
    });
  },
};
