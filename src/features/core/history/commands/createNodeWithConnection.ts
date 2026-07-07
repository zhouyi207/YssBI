import { NodeService, ConnectionService } from '@/services';
import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';
import { buildNodeDraft } from '@/features/core/dataStore/optimisticNodeDraft';
import { useNodeRegistryStore } from '@/features/core/nodeRegister/useNodeRegistryStore';
import { findAutoConnectPinIndex } from '@/shared/utils/pinCompatibility';
import { trackPending } from '@/features/core/sync/utils/echoSuppressor';
import { waitForPinOffset } from '@/features/core/canvas/pinOffsetWaiter';
import type { Pin } from '@/shared/types/domain/pin';
import type { CommandHandler } from '../types';
import { NODE_CREATE_ECHO_DOMAIN } from './createNode';
import { CONNECTION_ECHO_DOMAIN } from './connectPins';

/**
 * 从 pin 拖拽创建节点：作为单个用户操作（单个撤销项）一次性完成
 * 「新建节点 + 自动连线」，且节点与连线都在后端往返之前就乐观渲染，避免出现
 * 先有节点、隔一拍才出现连线的两段式卡顿。
 */
export interface CreateNodeWithConnectionArgs {
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
  /** 拖拽的源 pin（完整对象，用于计算自动连线的目标 pin） */
  sourcePin: Pin;
}

interface AutoDisconnectedEntry {
  fromPin: string;
  toPin: string;
}

export interface CreateNodeWithConnectionContext {
  nodeId: string;
  pinIds: string[];
  nodeType: string;
  x: number;
  y: number;
  params?: CreateNodeWithConnectionArgs['params'];
  sourcePinId: string;
  /** 实际建立连接的目标 pin；为 null 表示未找到兼容 pin、仅创建了节点 */
  targetPinId: string | null;
  autoDisconnectedList: AutoDisconnectedEntry[];
}

export const createNodeWithConnectionCommand: CommandHandler<
  CreateNodeWithConnectionArgs,
  CreateNodeWithConnectionContext
> = {
  async execute(graphId, args) {
    const store = useGraphDataStore.getState();
    const definition = useNodeRegistryStore.getState().getDefinition(args.nodeType);
    if (!definition) {
      throw new Error(`No node definition for type "${args.nodeType}"`);
    }

    const { node, pins, effectiveDefinition } = buildNodeDraft(
      graphId,
      args.nodeType,
      definition,
      args.x,
      args.y,
      args.params,
    );
    const nodeId = node.id;
    const pinIds = pins.map((p) => p.id);
    store.applyNodeDraft(graphId, node, pins);

    let targetPinId: string | null = null;
    let connDraft: ReturnType<typeof store.applyConnectionDraft> = null;
    const matchIdx = findAutoConnectPinIndex(effectiveDefinition.pinSlots, args.sourcePin);
    if (matchIdx >= 0 && matchIdx < pinIds.length) {
      targetPinId = pinIds[matchIdx];
      connDraft = store.applyConnectionDraft(args.sourcePin.id, targetPinId, graphId);
    }

    let finalX = args.x;
    let finalY = args.y;
    if (targetPinId) {
      const offset = await waitForPinOffset(graphId, targetPinId);
      if (offset) {
        finalX = args.x - offset.x;
        finalY = args.y - offset.y;
        store.updateNode(nodeId, { position: { x: finalX, y: finalY } }, graphId);
      }
    }

    try {
      await trackPending(
        NODE_CREATE_ECHO_DOMAIN,
        [nodeId],
        NodeService.createNodeWithId(
          graphId,
          nodeId,
          pinIds,
          args.nodeType,
          finalX,
          finalY,
          args.params,
        ),
      );
    } catch (error) {
      if (connDraft) store.revertConnectionDraft(connDraft, graphId);
      store.revertNodeDraft(nodeId, graphId);
      throw error;
    }

    let autoDisconnectedList: AutoDisconnectedEntry[] = [];
    if (connDraft && targetPinId) {
      const keys = [connDraft.connectionId, ...connDraft.disconnectedIds];
      try {
        const result = await trackPending(
          CONNECTION_ECHO_DOMAIN,
          keys,
          ConnectionService.connectPins(graphId, args.sourcePin.id, targetPinId),
        );
        autoDisconnectedList = result.autoDisconnected;
      } catch {
        store.revertConnectionDraft(connDraft, graphId);
        targetPinId = null;
      }
    }

    return {
      nodeId,
      pinIds,
      nodeType: args.nodeType,
      x: finalX,
      y: finalY,
      params: args.params,
      sourcePinId: args.sourcePin.id,
      targetPinId,
      autoDisconnectedList,
    };
  },

  async undo(graphId, context) {
    await NodeService.deleteNode(graphId, context.nodeId);
    for (const entry of context.autoDisconnectedList) {
      await ConnectionService.connectPins(graphId, entry.fromPin, entry.toPin);
    }
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
    if (context.targetPinId) {
      await ConnectionService.connectPins(graphId, context.sourcePinId, context.targetPinId);
    }
  },
};
