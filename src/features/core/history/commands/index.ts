import type { CommandHandler, CommandType } from '../types';
import { moveNodesCommand } from './moveNodes';
import { setPinValueCommand } from './setPinValue';
import { connectPinsCommand } from './connectPins';
import { disconnectPinCommand } from './disconnectPin';
import { createNodeCommand } from './createNode';
import { createNodeWithConnectionCommand } from './createNodeWithConnection';
import { deleteNodesCommand } from './deleteNodes';
import { batchCreateCommand } from './composite';
import { addRepeatablePinCommand, removeRepeatablePinCommand } from './repeatablePin';

export const commandRegistry: Record<string, CommandHandler<any, any>> = {
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

export function getCommandHandler(type: CommandType): CommandHandler<any, any> {
  const handler = commandRegistry[type];
  if (!handler) {
    throw new Error(`[CommandRegistry] Unknown command type: ${type}`);
  }
  return handler;
}

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
