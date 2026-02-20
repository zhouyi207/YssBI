import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';
import { NodeService } from '@/services';
import type { CommandHandler } from '../types';

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
      store.batchUpdateNodePositions(positions);
      NodeService.updateNodePositions(graphId, positions).catch((e) =>
        console.warn('[MoveNodes] updateNodePositions failed:', e),
      );
    }

    return { updates };
  },

  async undo(graphId, context) {
    const store = useGraphDataStore.getState();
    const positions = context.updates.map((u) => ({ nodeId: u.nodeId, x: u.oldX, y: u.oldY }));
    store.batchUpdateNodePositions(positions);
    await NodeService.updateNodePositions(graphId, positions);
  },

  async redo(graphId, context) {
    const store = useGraphDataStore.getState();
    const positions = context.updates.map((u) => ({ nodeId: u.nodeId, x: u.newX, y: u.newY }));
    store.batchUpdateNodePositions(positions);
    await NodeService.updateNodePositions(graphId, positions);
  },
};
