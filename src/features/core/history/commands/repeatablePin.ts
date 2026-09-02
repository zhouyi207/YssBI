import { useGraphProjectionStore } from "@/features/core/dataStore/graphProjectionStore";
import type { CommandHandler, GraphDraftCommandResult } from "../types";
import { executeGraphDraftIntent } from "./executeGraphDraftIntent";

export interface AddRepeatablePinArgs {
  nodeId: string;
  template: string;
}

export const addRepeatablePinCommand: CommandHandler<
  AddRepeatablePinArgs,
  GraphDraftCommandResult
> = {
  execute(graphPath, args) {
    return executeGraphDraftIntent(graphPath, {
      type: "addPortInstance",
      payload: { nodeId: args.nodeId, template: args.template, order: null },
    });
  },
};

export interface RemoveRepeatablePinArgs {
  nodeId: string;
  pinId: string;
}

export const removeRepeatablePinCommand: CommandHandler<
  RemoveRepeatablePinArgs,
  GraphDraftCommandResult
> = {
  execute(graphPath, args) {
    const pin = useGraphProjectionStore.getState().getGraphPin(graphPath, args.pinId);
    if (!pin || pin.address.kind !== "instance") {
      throw new Error(`Port '${args.pinId}' is not a removable port instance`);
    }
    if (pin.address.nodeId !== args.nodeId) {
      throw new Error(`Port '${args.pinId}' does not belong to node '${args.nodeId}'`);
    }
    return executeGraphDraftIntent(graphPath, {
      type: "removePortInstance",
      payload: { address: pin.address },
    });
  },
};
