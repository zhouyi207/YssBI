import type { CommandType } from '../types';
import type { AvailableCommandType, CommandHandlerMap } from './registryTypes';
import { moveNodesCommand } from './moveNodes';
import { setPinValueCommand } from './setPinValue';
import { connectPinsCommand } from './connectPins';
import { disconnectPinCommand } from './disconnectPin';
import { deleteNodesCommand } from './deleteNodes';
import { addRepeatablePinCommand, removeRepeatablePinCommand } from './repeatablePin';

export const commandRegistry: CommandHandlerMap = {
  MoveNodes: moveNodesCommand,
  SetPinValue: setPinValueCommand,
  ConnectPins: connectPinsCommand,
  DisconnectPin: disconnectPinCommand,
  DeleteNodes: deleteNodesCommand,
  AddRepeatablePin: addRepeatablePinCommand,
  RemoveRepeatablePin: removeRepeatablePinCommand,
};

export function getCommandHandler(type: CommandType) {
  return commandRegistry[type as AvailableCommandType];
}

export type { CommandHandlerMap } from './registryTypes';
export type { MoveNodesArgs } from './moveNodes';
export type { SetPinValueArgs } from './setPinValue';
export type { ConnectPinsArgs } from './connectPins';
export type { DisconnectPinArgs } from './disconnectPin';
export type { DeleteNodesArgs } from './deleteNodes';
export type { AddRepeatablePinArgs, RemoveRepeatablePinArgs } from './repeatablePin';
