import type { CommandHandler, GraphMutationCommandResult } from '../types';
import { executeGraphIntent } from './executeGraphIntent';

export interface DeleteNodesArgs {
  nodeIds: string[];
}

export const deleteNodesCommand: CommandHandler<DeleteNodesArgs, GraphMutationCommandResult> = {
  execute(graphPath, args) {
    if (args.nodeIds.length === 0) return false;
    return executeGraphIntent(graphPath, {
      type: 'deleteNodes',
      payload: { nodeIds: args.nodeIds },
    });
  },
};
