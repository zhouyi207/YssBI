import type { ApplyGraphDraftMutationOutcome } from "@/features/application/graphDraft/graphDraftCoordinator";

export type GraphEditOutcome =
  | ApplyGraphDraftMutationOutcome
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
