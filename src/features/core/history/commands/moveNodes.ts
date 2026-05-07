import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';
import { NodeService } from '@/services';
import type { CommandHandler } from '../types';
import { logger } from '@/utils/appLogger';
import { trackPending } from '@/features/core/sync/utils/echoSuppressor';

export const NODE_POSITION_ECHO_DOMAIN = 'node-position';

export interface MoveNodesArgs {
  nodeIds: string[];
  delta: { x: number; y: number };
}

export interface MoveNodesContext {
  updates: Array<{
    nodeId: string;
    oldX: number;
    oldY: number;
    newX: number;
    newY: number;
  }>;
}

export const moveNodesCommand: CommandHandler<MoveNodesArgs, MoveNodesContext> = {
  execute(graphId, args) {
    const store = useGraphDataStore.getState();
    const updates: MoveNodesContext['updates'] = [];

    for (const id of args.nodeIds) {
      const node = store.nodes[id];
      if (node?.position) {
        updates.push({
          nodeId: id,
          oldX: node.position.x,
          oldY: node.position.y,
          newX: node.position.x + args.delta.x,
          newY: node.position.y + args.delta.y,
        });
      }
    }

    if (updates.length > 0) {
      const positions = updates.map((u) => ({ nodeId: u.nodeId, x: u.newX, y: u.newY }));
      const ids = positions.map((p) => p.nodeId);
      store.batchUpdateNodePositions(positions);
      trackPending(
        NODE_POSITION_ECHO_DOMAIN,
        ids,
        NodeService.updateNodePositions(graphId, positions),
      ).catch((e) =>
        logger.graph.warn(`updateNodePositions failed: ${e instanceof Error ? e.message : String(e)}`, 'MoveNodes'),
      );
    }

    return { updates };
  },

  async undo(graphId, context) {
    const store = useGraphDataStore.getState();
    const positions = context.updates.map((u) => ({ nodeId: u.nodeId, x: u.oldX, y: u.oldY }));
    const ids = positions.map((p) => p.nodeId);
    store.batchUpdateNodePositions(positions);
    await trackPending(
      NODE_POSITION_ECHO_DOMAIN,
      ids,
      NodeService.updateNodePositions(graphId, positions),
    );
  },

  async redo(graphId, context) {
    const store = useGraphDataStore.getState();
    const positions = context.updates.map((u) => ({ nodeId: u.nodeId, x: u.newX, y: u.newY }));
    const ids = positions.map((p) => p.nodeId);
    store.batchUpdateNodePositions(positions);
    await trackPending(
      NODE_POSITION_ECHO_DOMAIN,
      ids,
      NodeService.updateNodePositions(graphId, positions),
    );
  },
};
