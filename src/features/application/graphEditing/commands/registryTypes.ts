import type { CommandHandler, GraphEditOutcome } from "../types";
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
  MoveNodes: CommandHandler<MoveNodesArgs, GraphEditOutcome>;
  SetPinValue: CommandHandler<SetPinValueArgs, GraphEditOutcome>;
  ConnectPins: CommandHandler<ConnectPinsArgs, GraphEditOutcome>;
  DisconnectPort: CommandHandler<DisconnectPortArgs, GraphEditOutcome>;
  DisconnectNode: CommandHandler<DisconnectNodeArgs, GraphEditOutcome>;
  DisconnectConnections: CommandHandler<DisconnectConnectionsArgs, GraphEditOutcome>;
  InsertReroute: CommandHandler<InsertRerouteArgs, GraphEditOutcome>;
  MoveConnections: CommandHandler<MoveConnectionsArgs, GraphEditOutcome>;
  DeleteNodes: CommandHandler<DeleteNodesArgs, GraphEditOutcome>;
  DuplicateSubgraph: CommandHandler<DuplicateSubgraphArgs, GraphEditOutcome>;
  InsertSubgraph: CommandHandler<InsertSubgraphArgs, GraphEditOutcome>;
  AddPortInstance: CommandHandler<AddPortInstanceArgs, GraphEditOutcome>;
  MovePortInstance: CommandHandler<MovePortInstanceArgs, GraphEditOutcome>;
  RemovePortInstance: CommandHandler<RemovePortInstanceArgs, GraphEditOutcome>;
}

export type AvailableCommandType = keyof CommandHandlerMap;
