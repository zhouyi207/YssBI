import type { CommandHandler, GraphMutationCommandResult } from '../types';
import type { MoveNodesArgs } from './moveNodes';
import type { SetPinValueArgs } from './setPinValue';
import type { ConnectPinsArgs } from './connectPins';
import type { DisconnectPortArgs } from './disconnectPin';
import type { DisconnectNodeArgs } from './disconnectNode';
import type { DisconnectConnectionsArgs } from './disconnectConnections';
import type { InsertRerouteArgs } from './insertReroute';
import type { MoveConnectionsArgs } from './moveConnections';
import type { DeleteNodesArgs } from './deleteNodes';
import type { DuplicateSubgraphArgs } from './duplicateSubgraph';
import type { InsertSubgraphArgs } from './insertSubgraph';
import type { AddRepeatablePinArgs, RemoveRepeatablePinArgs } from './repeatablePin';

export interface CommandHandlerMap {
  MoveNodes: CommandHandler<MoveNodesArgs, GraphMutationCommandResult>;
  SetPinValue: CommandHandler<SetPinValueArgs, GraphMutationCommandResult>;
  ConnectPins: CommandHandler<ConnectPinsArgs, GraphMutationCommandResult>;
  DisconnectPort: CommandHandler<DisconnectPortArgs, GraphMutationCommandResult>;
  DisconnectNode: CommandHandler<DisconnectNodeArgs, GraphMutationCommandResult>;
  DisconnectConnections: CommandHandler<DisconnectConnectionsArgs, GraphMutationCommandResult>;
  InsertReroute: CommandHandler<InsertRerouteArgs, GraphMutationCommandResult>;
  MoveConnections: CommandHandler<MoveConnectionsArgs, GraphMutationCommandResult>;
  DeleteNodes: CommandHandler<DeleteNodesArgs, GraphMutationCommandResult>;
  DuplicateSubgraph: CommandHandler<DuplicateSubgraphArgs, GraphMutationCommandResult>;
  InsertSubgraph: CommandHandler<InsertSubgraphArgs, GraphMutationCommandResult>;
  AddRepeatablePin: CommandHandler<AddRepeatablePinArgs, GraphMutationCommandResult>;
  RemoveRepeatablePin: CommandHandler<RemoveRepeatablePinArgs, GraphMutationCommandResult>;
}

export type AvailableCommandType = keyof CommandHandlerMap;
