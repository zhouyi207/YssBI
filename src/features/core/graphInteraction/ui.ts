import { useSyncExternalStore } from 'react';

import type { DeepReadonly } from '@/features/core/projection/deepReadonly';
import {
  getCanvasInteraction,
  getPositionOverride,
  useGraphInteractionStore,
  type CanvasInteraction,
  type GraphInteractionState,
  type NodeDragSession,
  type NodePosition,
} from './graphInteractionStore';
import type { GraphPath, NodeId } from '@/shared/types';

export interface GraphInteractionUiSnapshot {
  readonly positionOverrides: DeepReadonly<GraphInteractionState['positionOverrides']>;
  readonly interactions: DeepReadonly<GraphInteractionState['interactions']>;
}

export interface GraphInteractionUiCapability {
  readonly getSnapshot: () => DeepReadonly<GraphInteractionUiSnapshot>;
  readonly subscribe: (listener: () => void) => () => void;
  readonly getCanvasInteraction: (
    graphPath: GraphPath,
    groupId: string,
  ) => DeepReadonly<CanvasInteraction>;
  readonly getPositionOverride: (
    graphPath: GraphPath,
    nodeId: NodeId,
  ) => DeepReadonly<NodePosition> | undefined;
  readonly startInteraction: (
    graphPath: GraphPath,
    interaction: Exclude<CanvasInteraction, { type: 'idle' }>,
  ) => void;
  readonly updateInteraction: (
    graphPath: GraphPath,
    groupId: string,
    updater: (interaction: CanvasInteraction) => CanvasInteraction,
  ) => void;
  readonly updateNodeDragFrame: (
    graphPath: GraphPath,
    groupId: string,
    positions: Record<NodeId, NodePosition>,
    session: NodeDragSession,
  ) => void;
  readonly finishInteraction: (graphPath: GraphPath, groupId: string) => CanvasInteraction['type'];
  readonly cancelInteraction: (graphPath: GraphPath, groupId: string) => CanvasInteraction['type'];
  readonly setPositionOverride: (
    graphPath: GraphPath,
    nodeId: NodeId,
    position: NodePosition,
  ) => void;
  readonly clearPositionOverrides: (graphPath: GraphPath, nodeIds?: NodeId[]) => void;
  readonly clearGraphInteraction: (graphPath: GraphPath) => void;
}

function cloneAndFreeze<T>(value: T): T {
  if (Array.isArray(value)) return Object.freeze(value.map(cloneAndFreeze)) as T;
  if (value === null || typeof value !== 'object') return value;
  return Object.freeze(Object.fromEntries(
    Object.entries(value as Record<string, unknown>)
      .map(([key, nested]) => [key, cloneAndFreeze(nested)]),
  )) as T;
}

function buildSnapshot(): DeepReadonly<GraphInteractionUiSnapshot> {
  const state = useGraphInteractionStore.getState();
  return Object.freeze({
    positionOverrides: cloneAndFreeze(state.positionOverrides),
    interactions: cloneAndFreeze(state.interactions),
  });
}

let currentSnapshot = buildSnapshot();
const listeners = new Set<() => void>();

function refreshSnapshot(): void {
  currentSnapshot = buildSnapshot();
  for (const listener of listeners) listener();
}

useGraphInteractionStore.subscribe(refreshSnapshot);

export function getGraphInteractionUiSnapshot(): DeepReadonly<GraphInteractionUiSnapshot> {
  return currentSnapshot;
}

export function subscribeGraphInteractionUi(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function useGraphInteractionUi<T>(
  selector: (snapshot: DeepReadonly<GraphInteractionUiSnapshot>) => T,
): T {
  const snapshot = useSyncExternalStore(
    subscribeGraphInteractionUi,
    getGraphInteractionUiSnapshot,
    getGraphInteractionUiSnapshot,
  );
  return selector(snapshot);
}

export const graphInteractionUi: GraphInteractionUiCapability = {
  getSnapshot: getGraphInteractionUiSnapshot,
  subscribe: subscribeGraphInteractionUi,
  getCanvasInteraction: (graphPath, groupId) => cloneAndFreeze(
    getCanvasInteraction(useGraphInteractionStore.getState(), graphPath, groupId),
  ),
  getPositionOverride: (graphPath, nodeId) => {
    const position = getPositionOverride(
      useGraphInteractionStore.getState(),
      graphPath,
      nodeId,
    );
    return position ? Object.freeze({ ...position }) : undefined;
  },
  startInteraction: (graphPath, interaction) =>
    useGraphInteractionStore.getState().startInteraction(graphPath, interaction),
  updateInteraction: (graphPath, groupId, updater) =>
    useGraphInteractionStore.getState().updateInteraction(graphPath, groupId, updater),
  updateNodeDragFrame: (graphPath, groupId, positions, session) =>
    useGraphInteractionStore.getState().updateNodeDragFrame(
      graphPath,
      groupId,
      positions,
      session,
    ),
  finishInteraction: (graphPath, groupId) =>
    useGraphInteractionStore.getState().finishInteraction(graphPath, groupId),
  cancelInteraction: (graphPath, groupId) =>
    useGraphInteractionStore.getState().cancelInteraction(graphPath, groupId),
  setPositionOverride: (graphPath, nodeId, position) =>
    useGraphInteractionStore.getState().setPositionOverride(graphPath, nodeId, position),
  clearPositionOverrides: (graphPath, nodeIds) =>
    useGraphInteractionStore.getState().clearPositionOverrides(graphPath, nodeIds),
  clearGraphInteraction: (graphPath) =>
    useGraphInteractionStore.getState().clearGraphInteraction(graphPath),
};
