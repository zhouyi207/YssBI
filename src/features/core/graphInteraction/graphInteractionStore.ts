import { create } from 'zustand';
import type { GraphPath, NodeId, PinData } from '@/shared/types';
import type { ConnectionFeedback } from '@/features/core/canvas/connectionInteraction';

export interface NodePosition { x: number; y: number }
export interface PanSession { groupId: string; pointerId: number; startX: number; startY: number; lastX: number; lastY: number; moved: boolean }
export interface SelectionSession { groupId: string; pointerId: number; startX: number; startY: number; currentX: number; currentY: number; baseNodeIds: readonly string[] }
export interface NodeDragSession { groupId: string; pointerId: number; nodeId: string; lastX: number; lastY: number; moved: boolean; nodeIds: string[]; delta: NodePosition }
export interface ConnectionSession {
  groupId: string;
  pointerId: number;
  graphPath: GraphPath;
  source: PinData;
  screenX: number;
  screenY: number;
  worldX: number;
  worldY: number;
  hoveredTarget: PinData | null;
  snappedTarget: PinData | null;
  snappedWorld: NodePosition | null;
  feedback: ConnectionFeedback | null;
}
export interface PendingNodeCreationSession {
  groupId: string;
  graphPath: GraphPath;
  source: PinData | null;
  screenX: number;
  screenY: number;
}

export type CanvasInteraction =
  | { type: 'idle' }
  | { type: 'panning'; session: PanSession }
  | { type: 'selecting'; session: SelectionSession }
  | { type: 'draggingNodes'; session: NodeDragSession }
  | { type: 'drawingConnection'; session: ConnectionSession }
  | { type: 'movingConnections'; session: ConnectionSession }
  | { type: 'pendingNodeCreation'; session: PendingNodeCreationSession };

export interface CanvasInteractionScope { graphPath: GraphPath; groupId: string; pointerId: number }

export const IDLE_CANVAS_INTERACTION: CanvasInteraction = { type: 'idle' };

export function getCanvasInteraction(
  state: Pick<GraphInteractionState, 'interactions'>,
  graphPath: GraphPath,
  groupId: string,
): CanvasInteraction {
  const interaction = state.interactions[graphPath];
  return interaction?.type !== 'idle' && interaction?.session.groupId === groupId
    ? interaction
    : IDLE_CANVAS_INTERACTION;
}

export interface GraphInteractionState {
  positionOverrides: Record<GraphPath, Record<NodeId, NodePosition>>;
  interactions: Record<string, CanvasInteraction>;
  startInteraction(graphPath: GraphPath, interaction: Exclude<CanvasInteraction, { type: 'idle' }>): void;
  updateInteraction(graphPath: GraphPath, groupId: string, updater: (interaction: CanvasInteraction) => CanvasInteraction): void;
  updateNodeDragFrame(
    graphPath: GraphPath,
    groupId: string,
    positions: Record<NodeId, NodePosition>,
    session: NodeDragSession,
  ): void;
  finishInteraction(graphPath: GraphPath, groupId: string): CanvasInteraction['type'];
  cancelInteraction(graphPath: GraphPath, groupId: string): CanvasInteraction['type'];
  setPositionOverride(graphPath: GraphPath, nodeId: NodeId, position: NodePosition): void;
  clearPositionOverrides(graphPath: GraphPath, nodeIds?: NodeId[]): void;
  clearGraphInteraction(graphPath: GraphPath): void;
}

export const useGraphInteractionStore = create<GraphInteractionState>((set, get) => ({
  positionOverrides: {},
  interactions: {},
  startInteraction: (graphPath, interaction) => set((state) => ({
    interactions: { ...state.interactions, [graphPath]: interaction },
  })),
  updateInteraction: (graphPath, groupId, updater) => set((state) => {
    const current = state.interactions[graphPath] ?? { type: 'idle' };
    if (current.type !== 'idle' && current.session.groupId !== groupId) return state;
    return { interactions: { ...state.interactions, [graphPath]: updater(current) } };
  }),
  updateNodeDragFrame: (graphPath, groupId, positions, session) => set((state) => {
    const current = getCanvasInteraction(state, graphPath, groupId);
    if (current.type !== 'draggingNodes') return state;
    return {
      interactions: {
        ...state.interactions,
        [graphPath]: { type: 'draggingNodes', session },
      },
      positionOverrides: {
        ...state.positionOverrides,
        [graphPath]: {
          ...state.positionOverrides[graphPath],
          ...positions,
        },
      },
    };
  }),
  finishInteraction: (graphPath, groupId) => {
    const previous = getCanvasInteraction(get(), graphPath, groupId);
    if (previous.type === 'idle') return 'idle';
    set((state) => ({ interactions: { ...state.interactions, [graphPath]: { type: 'idle' } } }));
    return previous.type;
  },
  cancelInteraction: (graphPath, groupId) => {
    const previous = getCanvasInteraction(get(), graphPath, groupId);
    if (previous.type === 'idle') return 'idle';
    set((state) => ({
      interactions: { ...state.interactions, [graphPath]: { type: 'idle' } },
      positionOverrides: Object.fromEntries(Object.entries(state.positionOverrides).filter(([path]) => path !== graphPath)),
    }));
    return previous.type;
  },
  setPositionOverride: (graphPath, nodeId, position) => set((state) => ({
    positionOverrides: {
      ...state.positionOverrides,
      [graphPath]: { ...state.positionOverrides[graphPath], [nodeId]: position },
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
  clearGraphInteraction: (graphPath) => set((state) => {
    const interactions = { ...state.interactions };
    delete interactions[graphPath];
    return {
      interactions,
      positionOverrides: Object.fromEntries(Object.entries(state.positionOverrides).filter(([path]) => path !== graphPath)),
    };
  }),
}));

export function getPositionOverride(
  state: Pick<GraphInteractionState, 'positionOverrides'>,
  graphPath: GraphPath,
  nodeId: NodeId,
): NodePosition | undefined {
  return state.positionOverrides[graphPath]?.[nodeId];
}
