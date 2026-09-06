import { useGraphProjectionStore } from "@/features/core/dataStore/graphProjectionStore";
import type { CommandHandler, GraphEditOutcome } from "../types";
import { applyGraphDraftMutation } from "../../graphDraft/graphDraftCoordinator";

export interface ConnectPinsArgs {
  pinA: string;
  pinB: string;
}

export const connectPinsCommand: CommandHandler<ConnectPinsArgs, GraphEditOutcome> = {
  execute(graphPath, args) {
    const store = useGraphProjectionStore.getState();
    const pinA = store.getGraphPin(graphPath, args.pinA);
    const pinB = store.getGraphPin(graphPath, args.pinB);
    if (!pinA || !pinB) throw new Error("Cannot connect ports missing from the projection");
    const output = pinA.direction === "output" ? pinA : pinB;
    const input = pinA.direction === "input" ? pinA : pinB;
    if (output.direction !== "output" || input.direction !== "input") {
      throw new Error("A connection requires one output port and one input port");
    }
    return applyGraphDraftMutation({
      graphPath,
      mutation: {
        type: "connect",
        payload: { output: output.address, input: input.address, order: null },
      },
    });
  },
};
