import { ConnectionService } from '@/services';
import type { CommandHandler } from '../types';
import { logger } from '@/utils/appLogger';

export interface DisconnectPinArgs {
  pinId: string;
}

export interface DisconnectPinContext {
  pinId: string;
  /** All connections that were removed */
  removedConnections: Array<{ fromPin: string; toPin: string }>;
}

export const disconnectPinCommand: CommandHandler<DisconnectPinArgs, DisconnectPinContext> = {
  async execute(graphId, args) {
    const removed = await ConnectionService.disconnectPin(graphId, args.pinId);

    return {
      pinId: args.pinId,
      removedConnections: removed,
    };
  },

  async undo(graphId, context) {
    for (const conn of context.removedConnections) {
      try {
        await ConnectionService.connectPins(graphId, conn.fromPin, conn.toPin);
      } catch (e) {
        logger.graph.warn(`Failed to reconnect: fromPin=${conn.fromPin}, toPin=${conn.toPin} - ${e instanceof Error ? e.message : String(e)}`, 'DisconnectPin');
      }
    }
  },

  async redo(graphId, context) {
    if (context.removedConnections.length > 0) {
      await ConnectionService.disconnectPin(graphId, context.pinId);
    }
  },
};
