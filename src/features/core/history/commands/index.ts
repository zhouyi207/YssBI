import type { CommandType } from '../types';
import type { CommandHandlerMap } from './registryTypes';
import { moveNodesCommand } from './moveNodes';
import { setPinValueCommand } from './setPinValue';
import { connectPinsCommand } from './connectPins';
import { disconnectPinCommand } from './disconnectPin';
import { createNodeCommand } from './createNode';
import { createNodeWithConnectionCommand } from './createNodeWithConnection';
import { deleteNodesCommand } from './deleteNodes';
import { batchCreateCommand } from './composite';
import { addRepeatablePinCommand, removeRepeatablePinCommand } from './repeatablePin';

export const commandRegistry: CommandHandlerMap = {
  MoveNodes: moveNodesCommand,
  SetPinValue: setPinValueCommand,
  ConnectPins: connectPinsCommand,
  DisconnectPin: disconnectPinCommand,
  CreateNode: createNodeCommand,
  CreateNodeWithConnection: createNodeWithConnectionCommand,
  DeleteNodes: deleteNodesCommand,
  Composite: batchCreateCommand,
  AddRepeatablePin: addRepeatablePinCommand,
  RemoveRepeatablePin: removeRepeatablePinCommand,
};

export function getCommandHandler<K extends CommandType>(type: K): CommandHandlerMap[K] {
  const handler = commandRegistry[type];
  if (!handler) {
    throw new Error(`[CommandRegistry] Unknown command type: ${type}`);
  }
  return handler;
}

export type { CommandArgsByType, CommandContextByType, CommandHandlerMap } from './registryTypes';
export type { MoveNodesArgs, MoveNodesContext } from './moveNodes';
export type { SetPinValueArgs, SetPinValueContext } from './setPinValue';
export type { ConnectPinsArgs, ConnectPinsContext } from './connectPins';
export type { DisconnectPinArgs, DisconnectPinContext } from './disconnectPin';
export type { CreateNodeArgs, CreateNodeContext } from './createNode';
export type {
  CreateNodeWithConnectionArgs,
  CreateNodeWithConnectionContext,
} from './createNodeWithConnection';
export type { DeleteNodesArgs, DeleteNodesContext } from './deleteNodes';
export type { BatchCreateArgs, BatchCreateContext } from './composite';
export type { AddRepeatablePinArgs, AddRepeatablePinContext, RemoveRepeatablePinArgs, RemoveRepeatablePinContext } from './repeatablePin';
