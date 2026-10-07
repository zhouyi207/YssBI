import { useResourceStore } from "@/features/core/resource/resourceStore";
import type { CommandHandler, GraphEditOutcome } from "../types";
import { applyGraphMutation } from "../../graphEditing/graphEditCoordinator";

export interface ConnectPinsArgs {
  pinA: string;
  pinB: string;
}

export const connectPinsCommand: CommandHandler<ConnectPinsArgs, GraphEditOutcome> = {
  execute(graphPath, args) {
    const store = useResourceStore.getState();
    const pinA = store.getGraphPin(graphPath, args.pinA);
    const pinB = store.getGraphPin(graphPath, args.pinB);
    if (!pinA || !pinB) throw new Error("Cannot connect ports missing from the projection");
    const output = pinA.direction === "output" ? pinA : pinB;
    const input = pinA.direction === "input" ? pinA : pinB;
    return applyGraphMutation({
      graphPath,
      mutation: {
        type: "connect",
        payload: { output: output.address, input: input.address, order: null },
      },
    });
  },
};
