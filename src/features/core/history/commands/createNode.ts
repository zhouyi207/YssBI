import { NodeService } from '@/services';
import type { CommandHandler } from '../types';

export interface CreateNodeArgs {
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
}

export interface CreateNodeContext {
  nodeId: string;
  pinIds: string[];
  nodeType: string;
  x: number;
  y: number;
  params?: CreateNodeArgs['params'];
}

export const createNodeCommand: CommandHandler<CreateNodeArgs, CreateNodeContext> = {
  async execute(graphId, args) {
    const result = await NodeService.createNode(
      graphId,
      args.nodeType,
      args.x,
      args.y,
      args.params,
    );

    return {
      nodeId: result.nodeId,
      pinIds: result.pinIds,
      nodeType: args.nodeType,
      x: args.x,
      y: args.y,
      params: args.params,
    };
  },

  async undo(graphId, context) {
    await NodeService.deleteNode(graphId, context.nodeId);
  },

  async redo(graphId, context) {
    await NodeService.createNodeWithId(
      graphId,
      context.nodeId,
      context.pinIds,
      context.nodeType,
      context.x,
      context.y,
      context.params,
    );
  },
};
