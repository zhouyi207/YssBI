import type { GraphDraftUpdateDto } from "@/shared/types/domain/editorMutation";

export type GraphDraftCommandResult =
  | { status: "applied"; result: GraphDraftUpdateDto }
  | { status: "noop"; result: GraphDraftUpdateDto }
  | { status: "stale"; result?: GraphDraftUpdateDto }
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
  | "RemovePortInstance";

export interface CommandHandler<TArgs = unknown, TResult = unknown> {
  execute(graphPath: string, args: TArgs): Promise<TResult> | TResult;
}
