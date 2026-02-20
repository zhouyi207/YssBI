import { NodeService } from '@/services';
import type { ClipboardSnapshot } from '@/features/core/editor/stores/useClipboardStore';
import type { CommandHandler } from '../types';

// ==================== Composite (Batch Create with Connections) ====================

export interface BatchCreateArgs {
  snapshot: ClipboardSnapshot;
}

export interface BatchCreateContext {
  createdNodeIds: string[];
  snapshot: ClipboardSnapshot;
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
  async execute(graphId, args) {
    const result = await NodeService.batchCreateWithConnections(
      graphId,
      snapshotToServiceEntries(args.snapshot),
      args.snapshot.internalConnections,
    );

    return {
      createdNodeIds: result.nodeIds,
      snapshot: args.snapshot,
    };
  },

  async undo(graphId, context) {
    if (context.createdNodeIds.length > 0) {
      await NodeService.batchDeleteNodes(graphId, context.createdNodeIds);
    }
  },

  async redo(graphId, context) {
    const result = await NodeService.batchCreateWithConnections(
      graphId,
      snapshotToServiceEntries(context.snapshot),
      context.snapshot.internalConnections,
    );
    context.createdNodeIds = result.nodeIds;
  },
};
