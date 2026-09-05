import type { CommandHandler, GraphDraftCommandResult } from "../types";
import type { MoveNodesArgs } from "./moveNodes";
import type { SetPinValueArgs } from "./setPinValue";
import type { ConnectPinsArgs } from "./connectPins";
import type { DisconnectPortArgs } from "./disconnectPin";
import type { DisconnectNodeArgs } from "./disconnectNode";
import type { DisconnectConnectionsArgs } from "./disconnectConnections";
import type { InsertRerouteArgs } from "./insertReroute";
import type { MoveConnectionsArgs } from "./moveConnections";
import type { DeleteNodesArgs } from "./deleteNodes";
import type { DuplicateSubgraphArgs } from "./duplicateSubgraph";
import type { InsertSubgraphArgs } from "./insertSubgraph";
import type {
  AddPortInstanceArgs,
  MovePortInstanceArgs,
  RemovePortInstanceArgs,
} from "./portInstance";

export interface CommandHandlerMap {
  MoveNodes: CommandHandler<MoveNodesArgs, GraphDraftCommandResult>;
  SetPinValue: CommandHandler<SetPinValueArgs, GraphDraftCommandResult>;
  ConnectPins: CommandHandler<ConnectPinsArgs, GraphDraftCommandResult>;
  DisconnectPort: CommandHandler<DisconnectPortArgs, GraphDraftCommandResult>;
  DisconnectNode: CommandHandler<DisconnectNodeArgs, GraphDraftCommandResult>;
  DisconnectConnections: CommandHandler<DisconnectConnectionsArgs, GraphDraftCommandResult>;
  InsertReroute: CommandHandler<InsertRerouteArgs, GraphDraftCommandResult>;
  MoveConnections: CommandHandler<MoveConnectionsArgs, GraphDraftCommandResult>;
  DeleteNodes: CommandHandler<DeleteNodesArgs, GraphDraftCommandResult>;
  DuplicateSubgraph: CommandHandler<DuplicateSubgraphArgs, GraphDraftCommandResult>;
  InsertSubgraph: CommandHandler<InsertSubgraphArgs, GraphDraftCommandResult>;
  AddPortInstance: CommandHandler<AddPortInstanceArgs, GraphDraftCommandResult>;
  MovePortInstance: CommandHandler<MovePortInstanceArgs, GraphDraftCommandResult>;
  RemovePortInstance: CommandHandler<RemovePortInstanceArgs, GraphDraftCommandResult>;
}

export type AvailableCommandType = keyof CommandHandlerMap;
