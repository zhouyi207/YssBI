import { useGraphProjectionStore } from "@/features/core/dataStore/graphProjectionStore";
import type { CommandHandler, GraphDraftCommandResult } from "../types";
import { executeGraphDraftIntent } from "./executeGraphDraftIntent";

export interface MoveConnectionsArgs {
  sourcePinId: string;
  targetPinId: string;
}

export const moveConnectionsCommand: CommandHandler<MoveConnectionsArgs, GraphDraftCommandResult> =
  {
    execute(graphPath, args) {
      const store = useGraphProjectionStore.getState();
      const source = store.getGraphPin(graphPath, args.sourcePinId);
      const target = store.getGraphPin(graphPath, args.targetPinId);
      if (!source || !target) return false;
      return executeGraphDraftIntent(graphPath, {
        type: "moveConnections",
        payload: { source: source.address, target: target.address },
      });
    },
  };
