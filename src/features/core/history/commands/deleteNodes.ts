import { NodeService } from '@/services';
import type { GraphUndoPatch } from '@/shared/types/dto/graphUndoPatch';
import type { CommandHandler } from '../types';

export interface DeleteNodesArgs {
  nodeIds: string[];
}

export interface DeleteNodesContext {
  patch: GraphUndoPatch;
}

export const deleteNodesCommand: CommandHandler<DeleteNodesArgs, DeleteNodesContext> = {
  async execute(graphPath, args) {
    const patch = await NodeService.batchDeleteNodes(graphPath, args.nodeIds);
    return { patch };
  },

  async undo(graphPath, context) {
    await NodeService.applyGraphPatch(graphPath, context.patch);
  },

  async redo(graphPath, context) {
    const nodeIds = context.patch.nodes.map((n) => n.id);
    await NodeService.batchDeleteNodes(graphPath, nodeIds);
  },
};
