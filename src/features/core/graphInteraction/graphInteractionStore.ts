import { create } from 'zustand';
import type { GraphPath, NodeId } from '@/shared/types';

export interface NodePosition {
  x: number;
  y: number;
}

export interface GraphInteractionState {
  positionOverrides: Record<GraphPath, Record<NodeId, NodePosition>>;
  setPositionOverride(graphPath: GraphPath, nodeId: NodeId, position: NodePosition): void;
  clearPositionOverrides(graphPath: GraphPath, nodeIds?: NodeId[]): void;
  clearGraphInteraction(graphPath: GraphPath): void;
}

export const useGraphInteractionStore = create<GraphInteractionState>((set) => ({
  positionOverrides: {},

  setPositionOverride: (graphPath, nodeId, position) => set((state) => ({
    positionOverrides: {
      ...state.positionOverrides,
      [graphPath]: {
        ...state.positionOverrides[graphPath],
        [nodeId]: position,
      },
    },
  })),

  clearPositionOverrides: (graphPath, nodeIds) => set((state) => {
    const graphOverrides = state.positionOverrides[graphPath];
    if (!graphOverrides) return state;

    const positionOverrides = { ...state.positionOverrides };
    if (!nodeIds) {
      delete positionOverrides[graphPath];
      return { positionOverrides };
    }

    const remaining = { ...graphOverrides };
    for (const nodeId of nodeIds) delete remaining[nodeId];
    if (Object.keys(remaining).length === 0) delete positionOverrides[graphPath];
    else positionOverrides[graphPath] = remaining;
    return { positionOverrides };
  }),

  clearGraphInteraction: (graphPath) => {
    useGraphInteractionStore.getState().clearPositionOverrides(graphPath);
  },
}));

export function getPositionOverride(
  state: Pick<GraphInteractionState, 'positionOverrides'>,
  graphPath: GraphPath,
  nodeId: NodeId,
): NodePosition | undefined {
  return state.positionOverrides[graphPath]?.[nodeId];
}
