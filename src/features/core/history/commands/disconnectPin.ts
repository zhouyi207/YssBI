import { ConnectionService, NodeService } from '@/services';
import type { GraphUndoPatch } from '@/services/graph/node/graphUndoPatch';
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
  async execute(graphId, args) {
    const result = await ConnectionService.disconnectPin(graphId, args.pinId);

    return {
      pinId: args.pinId,
      removedConnections: result.removedConnections,
      undoPatch: result.undoPatch,
    };
  },

  async undo(graphId, context) {
    await NodeService.applyGraphPatch(graphId, context.undoPatch);
  },

  async redo(graphId, context) {
    if (context.removedConnections.length > 0) {
      await ConnectionService.disconnectPin(graphId, context.pinId);
    }
  },
};
