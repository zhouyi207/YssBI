import { useGraphDataStore } from "@/features/core/dataStore/graphDataStore";
import type { CommandHandler, GraphMutationCommandResult } from "../types";
import { executeGraphIntent } from "./executeGraphIntent";

export interface MoveConnectionsArgs {
  sourcePinId: string;
  targetPinId: string;
}

export const moveConnectionsCommand: CommandHandler<
  MoveConnectionsArgs,
  GraphMutationCommandResult
> = {
  execute(graphPath, args) {
    const store = useGraphDataStore.getState();
    const source = store.getGraphPin(graphPath, args.sourcePinId);
    const target = store.getGraphPin(graphPath, args.targetPinId);
    if (!source?.address || !target?.address) return false;
    return executeGraphIntent(graphPath, {
      type: "moveConnections",
      payload: { source: source.address, target: target.address },
    });
  },
};
