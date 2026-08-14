import {
  getCanvasInteraction,
  useGraphInteractionStore,
  type CanvasInteraction,
} from '@/features/core/graphInteraction/graphInteractionStore';
import { useGestureStore } from '@/features/core/gesture/useGestureStore';
import { clearCanvasPointerScope } from './pointerScope';

type ActiveInteractionType = Exclude<CanvasInteraction['type'], 'idle'>;
interface CleanupScope {
  graphPath: string;
  groupId: string;
  interactionType: ActiveInteractionType;
}

const cleanups = new Map<string, Set<() => void>>();

function cleanupKey(scope: CleanupScope): string {
  return `${scope.graphPath}\u0000${scope.groupId}\u0000${scope.interactionType}`;
}

export function registerCanvasInteractionCleanup(scope: CleanupScope, cleanup: () => void): () => void {
  const key = cleanupKey(scope);
  const bucket = cleanups.get(key) ?? new Set<() => void>();
  bucket.add(cleanup);
  cleanups.set(key, bucket);
  return () => {
    bucket.delete(cleanup);
    if (bucket.size === 0) cleanups.delete(key);
  };
}

export function startCanvasInteraction(
  graphPath: string,
  interaction: Exclude<CanvasInteraction, { type: 'idle' }>,
): void {
  const current = useGraphInteractionStore.getState().interactions[graphPath];
  if (current?.type !== 'idle') {
    cancelCanvasInteraction(graphPath, current.session.groupId);
  }
  useGraphInteractionStore.getState().startInteraction(graphPath, interaction);
}

function runCleanupKey(key: string): void {
  const callbacks = cleanups.get(key);
  cleanups.delete(key);
  for (const cleanup of callbacks ?? []) cleanup();
}

export function cancelCanvasInteraction(graphPath: string, groupId: string): CanvasInteraction['type'] {
  const interaction = getCanvasInteraction(useGraphInteractionStore.getState(), graphPath, groupId);
  if (interaction.type !== 'idle') {
    const key = cleanupKey({ graphPath, groupId, interactionType: interaction.type });
    runCleanupKey(key);
  }
  const result = useGraphInteractionStore.getState().cancelInteraction(graphPath, groupId);
  clearCanvasPointerScope(graphPath);
  return result;
}

export function clearCanvasInteractionGraph(graphPath: string): void {
  for (const key of [...cleanups.keys()]) {
    if (key.startsWith(`${graphPath}\u0000`)) runCleanupKey(key);
  }
  useGraphInteractionStore.getState().clearGraphInteraction(graphPath);
  useGestureStore.getState().clearGesture(false);
  clearCanvasPointerScope(graphPath);
}

export function clearCanvasInteractionProject(): void {
  for (const key of [...cleanups.keys()]) runCleanupKey(key);
  useGraphInteractionStore.setState({ interactions: {}, positionOverrides: {} });
  useGestureStore.getState().clearGesture(false);
  clearCanvasPointerScope();
}

export function resetCanvasInteractionCleanupForTests(): void {
  cleanups.clear();
  clearCanvasPointerScope();
}
