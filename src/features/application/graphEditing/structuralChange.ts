import { useExecutionStore } from "@/features/core/execution";
import type { CommandType } from "./types";

const STRUCTURAL_COMMANDS: ReadonlySet<CommandType> = new Set([
  "DeleteNodes",
  "DuplicateSubgraph",
  "InsertSubgraph",
  "ConnectPins",
  "DisconnectPort",
  "DisconnectNode",
  "DisconnectConnections",
  "InsertReroute",
  "MoveConnections",
  "AddPortInstance",
  "MovePortInstance",
  "RemovePortInstance",
]);

export function notifyStructuralChange(type: CommandType, graphPath: string) {
  if (STRUCTURAL_COMMANDS.has(type)) {
    useExecutionStore.getState().markGraphDirty(graphPath);
  }
}
