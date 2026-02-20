import { ConnectionService } from '@/services';
import type { CommandHandler } from '../types';

export interface ConnectPinsArgs {
  pinA: string;
  pinB: string;
}

export interface ConnectPinsContext {
  pinA: string;
  pinB: string;
  /** Actual direction determined by backend */
  fromPin: string;
  toPin: string;
  /** Connection auto-disconnected from input pin (if any) */
  autoDisconnectedFrom: string | null;
  autoDisconnectedTo: string | null;
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
    };
  },

  async undo(graphId, context) {
    const connectionId = `${context.fromPin}->${context.toPin}`;
    await ConnectionService.deleteConnection(graphId, connectionId);

    if (context.autoDisconnectedFrom && context.autoDisconnectedTo) {
      await ConnectionService.connectPins(
        graphId,
        context.autoDisconnectedFrom,
        context.autoDisconnectedTo,
      );
    }
  },

  async redo(graphId, context) {
    await ConnectionService.connectPins(graphId, context.pinA, context.pinB);
  },
};
