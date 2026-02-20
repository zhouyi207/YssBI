import { NodeService } from '@/services';
import type { CommandHandler } from '../types';

// ==================== Paste ====================

export interface PasteNodesArgs {
  requests: Array<{
    nodeType: string;
    x: number;
    y: number;
    params?: {
      variableId?: string;
      variableName?: string;
      variableType?: string;
      subGraphId?: string;
      dataframeId?: string;
    };
  }>;
}

export interface PasteNodesContext {
  /** IDs of nodes created by paste — used for undo (delete all) */
  createdNodeIds: string[];
  /** Original request data — used for redo (recreate) */
  requests: PasteNodesArgs['requests'];
}

export const pasteNodesCommand: CommandHandler<PasteNodesArgs, PasteNodesContext> = {
  async execute(graphId, args) {
    const nodeIds = await NodeService.batchCreateNodes(graphId, args.requests);

    return {
      createdNodeIds: nodeIds,
      requests: args.requests,
    };
  },

  async undo(graphId, context) {
    if (context.createdNodeIds.length > 0) {
      await NodeService.batchDeleteNodes(graphId, context.createdNodeIds);
    }
  },

  async redo(graphId, context) {
    await NodeService.batchCreateNodes(graphId, context.requests);
  },
};
