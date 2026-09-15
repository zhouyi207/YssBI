import type { ApplyGraphMutationOutcome } from "@/features/application/graphEditing/graphEditCoordinator";

export type GraphEditOutcome =
  | ApplyGraphMutationOutcome
  | { status: "unavailable" }
  | { status: "failed" };

export type CommandType =
  | "MoveNodes"
  | "SetPinValue"
  | "ConnectPins"
  | "DisconnectPort"
  | "DisconnectNode"
  | "DisconnectConnections"
  | "InsertReroute"
  | "MoveConnections"
  | "DeleteNodes"
  | "DuplicateSubgraph"
  | "InsertSubgraph"
  | "AddPortInstance"
  | "MovePortInstance"
  | "RemovePortInstance";

export interface CommandHandler<TArgs = unknown, TResult = unknown> {
  execute(graphPath: string, args: TArgs): Promise<TResult> | TResult;
}
