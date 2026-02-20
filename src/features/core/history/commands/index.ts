import type { CommandHandler, CommandType } from '../types';
import { moveNodesCommand } from './moveNodes';
import { setPinValueCommand } from './setPinValue';
import { connectPinsCommand } from './connectPins';
import { disconnectPinCommand } from './disconnectPin';
import { createNodeCommand } from './createNode';
import { deleteNodesCommand } from './deleteNodes';
import { pasteNodesCommand } from './composite';

export const commandRegistry: Record<string, CommandHandler<any, any>> = {
  MoveNodes: moveNodesCommand,
  SetPinValue: setPinValueCommand,
  ConnectPins: connectPinsCommand,
  DisconnectPin: disconnectPinCommand,
  CreateNode: createNodeCommand,
  DeleteNodes: deleteNodesCommand,
  Composite: pasteNodesCommand,
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
export type { DeleteNodesArgs, DeleteNodesContext } from './deleteNodes';
export type { PasteNodesArgs, PasteNodesContext } from './composite';
