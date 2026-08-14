import type { CommandHandler, GraphMutationCommandResult } from '../types';
import { executeGraphIntent } from './executeGraphIntent';

export interface DisconnectNodeArgs {
  nodeId: string;
}

export const disconnectNodeCommand: CommandHandler<DisconnectNodeArgs, GraphMutationCommandResult> = {
  execute(graphPath, args) {
    return executeGraphIntent(graphPath, {
      type: 'disconnectNode',
      payload: { nodeId: args.nodeId },
    });
  },
};
