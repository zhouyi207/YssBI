import type { CommandHandler, GraphMutationCommandResult } from '../types';
import { executeGraphIntent } from './executeGraphIntent';

export interface InsertSubgraphArgs {
  snapshotJson: string;
  anchor: { x: number; y: number };
}

export const insertSubgraphCommand: CommandHandler<
  InsertSubgraphArgs,
  GraphMutationCommandResult
> = {
  execute(graphPath, args) {
    return executeGraphIntent(graphPath, {
      type: 'insertSubgraph',
      payload: {
        snapshotJson: args.snapshotJson,
        anchor: { ...args.anchor },
      },
    });
  },
};
