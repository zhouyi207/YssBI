import type { CommandHandler, GraphMutationCommandResult } from '../types';
import { executeGraphIntent } from './executeGraphIntent';

export interface DuplicateSubgraphArgs {
  nodeIds: string[];
  offset: { x: number; y: number };
}

export const duplicateSubgraphCommand: CommandHandler<
  DuplicateSubgraphArgs,
  GraphMutationCommandResult
> = {
  execute(graphPath, args) {
    if (args.nodeIds.length === 0) return false;
    return executeGraphIntent(graphPath, {
      type: 'duplicateSubgraph',
      payload: {
        nodeIds: [...args.nodeIds],
        offset: { ...args.offset },
      },
    });
  },
};
