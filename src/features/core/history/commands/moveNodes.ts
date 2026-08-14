import type { NodePositionDto } from '@/shared/types/dto/editorProjection';
import type { CommandHandler, GraphMutationCommandResult } from '../types';
import { executeGraphIntent } from './executeGraphIntent';

export interface MoveNodesArgs {
  positions: Array<{ nodeId: string; position: NodePositionDto }>;
}

export const moveNodesCommand: CommandHandler<MoveNodesArgs, GraphMutationCommandResult> = {
  execute(graphPath, args) {
    return executeGraphIntent(graphPath, {
      type: 'moveNodes',
      payload: { positions: args.positions },
    });
  },
};
