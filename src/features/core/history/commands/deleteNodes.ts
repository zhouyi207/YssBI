import { NodeService } from '@/services';
import type { GraphUndoPatch } from '@/services/graph/node/graphUndoPatch';
import type { CommandHandler } from '../types';

export interface DeleteNodesArgs {
  nodeIds: string[];
}

export interface DeleteNodesContext {
  patch: GraphUndoPatch;
}

export const deleteNodesCommand: CommandHandler<DeleteNodesArgs, DeleteNodesContext> = {
  async execute(graphId, args) {
    const patch = await NodeService.batchDeleteNodes(graphId, args.nodeIds);
    return { patch };
  },

  async undo(graphId, context) {
    await NodeService.applyGraphPatch(graphId, context.patch);
  },

  async redo(graphId, context) {
    const nodeIds = context.patch.nodes.map((n) => n.id);
    await NodeService.batchDeleteNodes(graphId, nodeIds);
  },
};
