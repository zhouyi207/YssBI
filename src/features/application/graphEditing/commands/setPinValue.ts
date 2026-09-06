import { useGraphProjectionStore } from "@/features/core/dataStore/graphProjectionStore";
import type { CommandHandler, GraphEditOutcome } from "../types";
import { applyGraphDraftMutation } from "../../graphDraft/graphDraftCoordinator";

export interface SetPinValueArgs {
  pinId: string;
  nodeId: string;
  newValue: unknown;
}

export const setPinValueCommand: CommandHandler<SetPinValueArgs, GraphEditOutcome> = {
  execute(graphPath, args) {
    const pin = useGraphProjectionStore.getState().getGraphPin(graphPath, args.pinId);
    if (!pin) throw new Error(`Port '${args.pinId}' is not projected`);
    if (pin.address.nodeId !== args.nodeId) {
      throw new Error(`Port '${args.pinId}' does not belong to node '${args.nodeId}'`);
    }
    return applyGraphDraftMutation({
      graphPath,
      mutation: {
        type: "setLiteral",
        payload: { address: pin.address, literal: args.newValue ?? null },
      },
    });
  },
};
