import { NodeService } from '@/services';
import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';
import { buildNodeDraft } from '@/features/core/dataStore/optimisticNodeDraft';
import { useNodeRegistryStore } from '@/features/core/nodeRegister/useNodeRegistryStore';
import { trackPending } from '@/features/core/sync/utils/echoSuppressor';
import type { NodeSpawnParams } from '@/shared/types/dto/nodeInstanceParams';
import type { CommandHandler } from '../types';

/** Echo 抑制域：create 期间后端回传的 NodeCreated 事件 key（按 nodeId） */
export const NODE_CREATE_ECHO_DOMAIN = 'nodeCreate';

export interface CreateNodeArgs {
  nodeType: string;
  x: number;
  y: number;
  params?: NodeSpawnParams;
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
  async execute(graphPath, args) {
    const store = useGraphDataStore.getState();
    const definition = useNodeRegistryStore.getState().getDefinition(args.nodeType);

    // 无定义（理论上不应发生）：退回非乐观路径，由后端生成 id。
    if (!definition) {
      const result = await NodeService.createNode(
        graphPath,
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
    }

    // 乐观插入：本地立即渲染，后端用相同 id 创建并通过 NodeCreated 回传对齐。
    const { node, pins } = buildNodeDraft(
      graphPath,
      args.nodeType,
      definition,
      args.x,
      args.y,
      args.params,
    );
    const nodeId = node.id;
    const pinIds = pins.map((p) => p.id);

    store.applyNodeDraft(graphPath, node, pins);

    try {
      await trackPending(
        NODE_CREATE_ECHO_DOMAIN,
        [nodeId],
        NodeService.createNodeWithId(
          graphPath,
          nodeId,
          pinIds,
          args.nodeType,
          args.x,
          args.y,
          args.params,
        ),
      );
    } catch (error) {
      store.revertNodeDraft(nodeId, graphPath);
      throw error;
    }

    return {
      nodeId,
      pinIds,
      nodeType: args.nodeType,
      x: args.x,
      y: args.y,
      params: args.params,
    };
  },

  async undo(graphPath, context) {
    await NodeService.deleteNode(graphPath, context.nodeId);
  },

  async redo(graphPath, context) {
    await NodeService.createNodeWithId(
      graphPath,
      context.nodeId,
      context.pinIds,
      context.nodeType,
      context.x,
      context.y,
      context.params,
    );
  },
};
