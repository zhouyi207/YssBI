import { ConnectionService, NodeService } from '@/services';
import type { GraphUndoPatch } from '@/shared/types/dto/graphUndoPatch';
import type { CommandHandler } from '../types';

export interface DisconnectPinArgs {
  pinId: string;
}

export interface DisconnectPinContext {
  pinId: string;
  removedConnections: Array<{ fromPin: string; toPin: string }>;
  undoPatch: GraphUndoPatch;
}

export const disconnectPinCommand: CommandHandler<DisconnectPinArgs, DisconnectPinContext> = {
  async execute(graphPath, args) {
    const result = await ConnectionService.disconnectPin(graphPath, args.pinId);

    return {
      pinId: args.pinId,
      removedConnections: result.removedConnections,
      undoPatch: result.undoPatch,
    };
  },

  async undo(graphPath, context) {
    await NodeService.applyGraphPatch(graphPath, context.undoPatch);
  },

  async redo(graphPath, context) {
    if (context.removedConnections.length > 0) {
      await ConnectionService.disconnectPin(graphPath, context.pinId);
    }
  },
};
