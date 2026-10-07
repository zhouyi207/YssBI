import { useResourceStore } from "@/features/core/resource/resourceStore";
import type { CommandHandler, GraphEditOutcome } from "../types";
import { applyGraphMutation } from "../../graphEditing/graphEditCoordinator";

export interface SetPinValueArgs {
  pinId: string;
  nodeId: string;
  newValue: unknown;
}

export const setPinValueCommand: CommandHandler<SetPinValueArgs, GraphEditOutcome> = {
  execute(graphPath, args) {
    const pin = useResourceStore.getState().getGraphPin(graphPath, args.pinId);
    if (!pin) throw new Error(`Port '${args.pinId}' is not projected`);
    if (pin.address.nodeId !== args.nodeId) {
      throw new Error(`Port '${args.pinId}' does not belong to node '${args.nodeId}'`);
    }
    return applyGraphMutation({
      graphPath,
      mutation: {
        type: "setLiteral",
        payload: { address: pin.address, literal: args.newValue ?? null },
      },
    });
  },
};
