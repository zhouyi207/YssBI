import type { CommandHandler } from '../types';
import type { MoveNodesArgs } from './moveNodes';
import type { SetPinValueArgs } from './setPinValue';
import type { ConnectPinsArgs } from './connectPins';
import type { DisconnectPinArgs } from './disconnectPin';
import type { DeleteNodesArgs } from './deleteNodes';
import type { AddRepeatablePinArgs, RemoveRepeatablePinArgs } from './repeatablePin';

export interface CommandHandlerMap {
  MoveNodes: CommandHandler<MoveNodesArgs>;
  SetPinValue: CommandHandler<SetPinValueArgs>;
  ConnectPins: CommandHandler<ConnectPinsArgs>;
  DisconnectPin: CommandHandler<DisconnectPinArgs>;
  DeleteNodes: CommandHandler<DeleteNodesArgs>;
  AddRepeatablePin: CommandHandler<AddRepeatablePinArgs>;
  RemoveRepeatablePin: CommandHandler<RemoveRepeatablePinArgs>;
}

export type AvailableCommandType = keyof CommandHandlerMap;
