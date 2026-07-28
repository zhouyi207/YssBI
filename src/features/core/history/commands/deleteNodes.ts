import type { CommandHandler } from '../types';
import { executeGraphIntent } from './executeGraphIntent';

export interface DeleteNodesArgs {
  nodeIds: string[];
}

export const deleteNodesCommand: CommandHandler<DeleteNodesArgs, boolean> = {
  async execute(graphPath, args) {
    if (args.nodeIds.length === 0) return false;
    for (const nodeId of args.nodeIds) {
      const outcome = await executeGraphIntent(graphPath, {
        type: 'deleteNode',
        payload: { nodeId },
      });
      if (outcome.status !== 'applied') return false;
    }
    return true;
  },
};
