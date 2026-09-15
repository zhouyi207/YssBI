import { useGraphProjectionStore } from "@/features/core/dataStore/graphProjectionStore";
import type { CommandHandler, GraphEditOutcome } from "../types";
import { applyGraphMutation } from "../../graphEditing/graphEditCoordinator";

export interface MoveConnectionsArgs {
  sourcePinId: string;
  targetPinId: string;
}

export const moveConnectionsCommand: CommandHandler<MoveConnectionsArgs, GraphEditOutcome> = {
  execute(graphPath, args) {
    const store = useGraphProjectionStore.getState();
    const source = store.getGraphPin(graphPath, args.sourcePinId);
    const target = store.getGraphPin(graphPath, args.targetPinId);
    if (!source || !target) return { status: "unavailable" };
    return applyGraphMutation({
      graphPath,
      mutation: {
        type: "moveConnections",
        payload: { source: source.address, target: target.address },
      },
    });
  },
};
