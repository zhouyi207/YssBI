import { ConnectionService } from '@/services';
import type { CommandHandler } from '../types';

export interface ConnectPinsArgs {
  pinA: string;
  pinB: string;
}

interface AutoDisconnectedEntry {
  fromPin: string;
  toPin: string;
}

export interface ConnectPinsContext {
  pinA: string;
  pinB: string;
  fromPin: string;
  toPin: string;
  /** @deprecated use autoDisconnectedList */
  autoDisconnectedFrom: string | null;
  autoDisconnectedTo: string | null;
  autoDisconnectedList: AutoDisconnectedEntry[];
}

export const connectPinsCommand: CommandHandler<ConnectPinsArgs, ConnectPinsContext> = {
  async execute(_graphId, args) {
    const result = await ConnectionService.connectPins(_graphId, args.pinA, args.pinB);

    return {
      pinA: args.pinA,
      pinB: args.pinB,
      fromPin: result.fromPin,
      toPin: result.toPin,
      autoDisconnectedFrom: result.autoDisconnectedFrom,
      autoDisconnectedTo: result.autoDisconnectedTo,
      autoDisconnectedList: result.autoDisconnected,
    };
  },

  async undo(graphId, context) {
    const connectionId = `${context.fromPin}->${context.toPin}`;
    await ConnectionService.deleteConnection(graphId, connectionId);

    const toRestore = context.autoDisconnectedList ?? [];
    for (const entry of toRestore) {
      await ConnectionService.connectPins(graphId, entry.fromPin, entry.toPin);
    }
  },

  async redo(graphId, context) {
    await ConnectionService.connectPins(graphId, context.pinA, context.pinB);
  },
};
