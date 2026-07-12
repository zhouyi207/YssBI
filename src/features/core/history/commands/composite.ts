import { NodeService } from '@/services';
import type { ClipboardSnapshot } from '@/features/core/editor/stores/useClipboardStore';
import type { GraphUndoPatch } from '@/shared/types/dto/graphUndoPatch';
import type { CommandHandler } from '../types';

export interface BatchCreateArgs {
  snapshot: ClipboardSnapshot;
}

export interface BatchCreateContext {
  undoPatch: GraphUndoPatch;
}

function snapshotToServiceEntries(snapshot: ClipboardSnapshot) {
  return snapshot.entries.map(e => ({
    nodeType: e.nodeType,
    x: e.position.x,
    y: e.position.y,
    params: e.params,
    pins: e.pins,
  }));
}

export const batchCreateCommand: CommandHandler<BatchCreateArgs, BatchCreateContext> = {
  async execute(graphPath, args) {
    const result = await NodeService.batchCreateWithConnections(
      graphPath,
      snapshotToServiceEntries(args.snapshot),
      args.snapshot.internalConnections,
    );

    return {
      undoPatch: result.undoPatch,
    };
  },

  async undo(graphPath, context) {
    const nodeIds = context.undoPatch.nodes.map((n) => n.id);
    if (nodeIds.length > 0) {
      await NodeService.batchDeleteNodes(graphPath, nodeIds);
    }
  },

  async redo(graphPath, context) {
    await NodeService.applyGraphPatch(graphPath, context.undoPatch);
  },
};
