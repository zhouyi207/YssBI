import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';
import { NodeService } from '@/services';
import type { CommandHandler } from '../types';

export interface DeleteNodesArgs {
  nodeIds: string[];
}

interface SavedNode {
  nodeId: string;
  nodeType: string;
  x: number;
  y: number;
  params: {
    variableId?: string;
    variableName?: string;
    variableType?: string;
    subGraphId?: string;
    dataframeId?: string;
  } | null;
  pins: Array<{ pinId: string; name: string; direction: string; userValue?: unknown }>;
}

export interface DeleteNodesContext {
  savedNodes: SavedNode[];
  savedConnections: Array<{ fromPin: string; toPin: string }>;
}

export const deleteNodesCommand: CommandHandler<DeleteNodesArgs, DeleteNodesContext> = {
  async execute(graphId, args) {
    const store = useGraphDataStore.getState();

    // Capture full state of nodes to be deleted BEFORE deletion
    const savedNodes: SavedNode[] = [];
    const savedConnectionSet = new Set<string>();
    const savedConnections: Array<{ fromPin: string; toPin: string }> = [];

    for (const nodeId of args.nodeIds) {
      const node = store.nodes[nodeId];
      if (!node) continue;

      const pinIds = store.nodePins[nodeId] ?? [];
      const pins: Array<{ pinId: string; name: string; direction: string; userValue?: unknown }> = [];

      for (const pinId of pinIds) {
        const pin = store.pins[pinId];
        if (pin) {
          pins.push({
            pinId: pin.id,
            name: pin.name,
            direction: pin.direction,
            userValue: pin.userValue,
          });

          // Capture connections for this pin
          const connIds = store.pinConnections[pinId] ?? [];
          for (const connId of connIds) {
            if (savedConnectionSet.has(connId)) continue;
            const conn = store.connections[connId];
            if (conn) {
              savedConnectionSet.add(connId);
              savedConnections.push({
                fromPin: conn.from,
                toPin: conn.to,
              });
            }
          }
        }
      }

      const params = node.paramsKind && node.paramsKind !== 'none'
        ? {
            variableId: node.variableId,
            variableName: node.variableName,
            variableType: node.variableType,
            subGraphId: node.subGraphId,
            dataframeId: node.dataframeId,
          }
        : null;

      savedNodes.push({
        nodeId: node.id,
        nodeType: node.nodeType,
        x: node.position.x,
        y: node.position.y,
        params,
        pins,
      });
    }

    await NodeService.batchDeleteNodes(graphId, args.nodeIds);

    return { savedNodes, savedConnections };
  },

  async undo(graphId, context) {
    await NodeService.restoreNodes(graphId, context.savedNodes, context.savedConnections);
  },

  async redo(graphId, context) {
    const nodeIds = context.savedNodes.map((n) => n.nodeId);
    await NodeService.batchDeleteNodes(graphId, nodeIds);
  },
};
