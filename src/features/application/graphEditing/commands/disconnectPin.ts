import { useGraphProjectionStore } from "@/features/core/dataStore/graphProjectionStore";
import type { CommandHandler, GraphEditOutcome } from "../types";
import { applyGraphDraftMutation } from "../../graphDraft/graphDraftCoordinator";

export interface DisconnectPortArgs {
  pinId: string;
}

export const disconnectPortCommand: CommandHandler<DisconnectPortArgs, GraphEditOutcome> = {
  execute(graphPath, args) {
    const pin = useGraphProjectionStore.getState().getGraphPin(graphPath, args.pinId);
    if (!pin) return { status: "unavailable" };
    return applyGraphDraftMutation({
      graphPath,
      mutation: {
        type: "disconnectPort",
        payload: { address: pin.address },
      },
    });
  },
};
