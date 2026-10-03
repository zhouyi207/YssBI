import { useResourceStore } from "@/features/core/resource/resourceStore";
import type { CommandHandler, GraphEditOutcome } from "../types";
import { applyGraphMutation } from "../../graphEditing/graphEditCoordinator";

export interface DisconnectPortArgs {
  pinId: string;
}

export const disconnectPortCommand: CommandHandler<DisconnectPortArgs, GraphEditOutcome> = {
  execute(graphPath, args) {
    const pin = useResourceStore.getState().getGraphPin(graphPath, args.pinId);
    if (!pin) return { status: "unavailable" };
    return applyGraphMutation({
      graphPath,
      mutation: {
        type: "disconnectPort",
        payload: { address: pin.address },
      },
    });
  },
};
