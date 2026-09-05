import type { GraphDraftTransformDto } from "@/shared/types/domain/editorMutation";

export type GraphDraftCommandResult =
  | { status: "applied"; result: GraphDraftTransformDto; insertedNodeIds: string[] }
  | { status: "noop"; result: GraphDraftTransformDto }
  | { status: "stale"; result?: GraphDraftTransformDto }
  | { status: "saving" }
  | { status: "rejected"; code: string }
  | false;

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
