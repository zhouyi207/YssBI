import type { CommandHandler, CommandType } from '../types';
import type { MoveNodesArgs, MoveNodesContext } from './moveNodes';
import type { SetPinValueArgs, SetPinValueContext } from './setPinValue';
import type { ConnectPinsArgs, ConnectPinsContext } from './connectPins';
import type { DisconnectPinArgs, DisconnectPinContext } from './disconnectPin';
import type { CreateNodeArgs, CreateNodeContext } from './createNode';
import type {
  CreateNodeWithConnectionArgs,
  CreateNodeWithConnectionContext,
} from './createNodeWithConnection';
import type { DeleteNodesArgs, DeleteNodesContext } from './deleteNodes';
import type { BatchCreateArgs, BatchCreateContext } from './composite';
import type {
  AddRepeatablePinArgs,
  AddRepeatablePinContext,
  RemoveRepeatablePinArgs,
  RemoveRepeatablePinContext,
} from './repeatablePin';

export interface CommandArgsByType {
  MoveNodes: MoveNodesArgs;
  SetPinValue: SetPinValueArgs;
  ConnectPins: ConnectPinsArgs;
  DisconnectPin: DisconnectPinArgs;
  CreateNode: CreateNodeArgs;
  CreateNodeWithConnection: CreateNodeWithConnectionArgs;
  DeleteNodes: DeleteNodesArgs;
  Composite: BatchCreateArgs;
  AddRepeatablePin: AddRepeatablePinArgs;
  RemoveRepeatablePin: RemoveRepeatablePinArgs;
}

export interface CommandContextByType {
  MoveNodes: MoveNodesContext;
  SetPinValue: SetPinValueContext;
  ConnectPins: ConnectPinsContext;
  DisconnectPin: DisconnectPinContext;
  CreateNode: CreateNodeContext;
  CreateNodeWithConnection: CreateNodeWithConnectionContext;
  DeleteNodes: DeleteNodesContext;
  Composite: BatchCreateContext;
  AddRepeatablePin: AddRepeatablePinContext;
  RemoveRepeatablePin: RemoveRepeatablePinContext;
}

export type CommandHandlerMap = {
  [K in CommandType]: CommandHandler<CommandArgsByType[K], CommandContextByType[K]>;
};
