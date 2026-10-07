import {
  getCanvasInteraction,
  IDLE_CANVAS_INTERACTION,
  useGraphInteractionStore,
  type CanvasInteraction,
} from "@/features/core/graphInteraction/graphInteractionStore";
import { useGestureStore } from "@/features/core/gesture/useGestureStore";
import { produce } from "immer";
import {
  captureProjectLifecycleState,
  isProjectLifecycleStateCurrent,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";

type ActiveInteractionType = Exclude<CanvasInteraction["type"], "idle">;
interface CleanupScope {
  graphPath: string;
  groupId: string;
  interactionType: ActiveInteractionType;
}

const cleanups = new Map<string, Set<() => void>>();

function cleanupKey(scope: CleanupScope): string {
  return `${scope.graphPath}\u0000${scope.groupId}\u0000${scope.interactionType}`;
}

export function registerCanvasInteractionCleanup(
  scope: CleanupScope,
  cleanup: () => void,
): () => void {
  const key = cleanupKey(scope);
  const bucket = cleanups.get(key) ?? new Set<() => void>();
  bucket.add(cleanup);
  cleanups.set(key, bucket);
  return () => {
    bucket.delete(cleanup);
    if (bucket.size === 0 && cleanups.get(key) === bucket) cleanups.delete(key);
  };
}

export function startCanvasInteraction(
  graphPath: string,
  interaction: Exclude<CanvasInteraction, { type: "idle" }>,
): Exclude<CanvasInteraction, { type: "idle" }> | null {
  const owner = captureProjectLifecycleState();
  const current = useGraphInteractionStore.getState().interactions[graphPath];
  if (current && current.type !== "idle") {
    const cancelled = cancelCanvasInteraction(graphPath, current.session.groupId);
    if (
      !isProjectLifecycleStateCurrent(owner) ||
      cancelled !== current.type ||
      useGraphInteractionStore.getState().interactions[graphPath] !== IDLE_CANVAS_INTERACTION
    )
      return null;
  }
  const installed = useGraphInteractionStore.getState().startInteraction(graphPath, interaction);
  return isProjectLifecycleStateCurrent(owner) &&
    useGraphInteractionStore.getState().interactions[graphPath] === installed
    ? installed
    : null;
}

export function cancelCanvasInteraction(
  graphPath: string,
  groupId: string,
): CanvasInteraction["type"] {
  const owner = captureProjectLifecycleState();
  const interaction = getCanvasInteraction(useGraphInteractionStore.getState(), graphPath, groupId);
  if (interaction.type === "idle") return "idle";
  const isCurrent = () =>
    isProjectLifecycleStateCurrent(owner) &&
    useGraphInteractionStore.getState().interactions[graphPath] === interaction;
  const key = cleanupKey({ graphPath, groupId, interactionType: interaction.type });
  const callbacks = cleanups.get(key);
  cleanups.delete(key);
  for (const cleanup of callbacks ?? []) {
    if (!isCurrent()) return "idle";
    cleanup();
  }
  if (!isCurrent()) return "idle";
  return useGraphInteractionStore.getState().cancelInteraction(graphPath, groupId);
}

export function clearCanvasInteractionGraph(graphPath: string, isCurrent?: () => boolean): void {
  clearCanvasInteractions(graphPath, isCurrent);
}

export function clearCanvasInteractionProject(): void {
  clearCanvasInteractions();
}

function clearCanvasInteractions(graphPath?: string, isCurrent?: () => boolean): void {
  const owner = captureProjectLifecycleState();
  const current = () => isProjectLifecycleStateCurrent(owner) && (!isCurrent || isCurrent());
  if (!current()) return;
  const interactions = useGraphInteractionStore.getState().interactions;
  const gesture = useGestureStore.getState();
  const captured = [...cleanups].filter(
    ([key]) => graphPath === undefined || key.startsWith(`${graphPath}\u0000`),
  );
  // Detach the entire old batch before callbacks can register a successor under any same key.
  for (const [key] of captured) cleanups.delete(key);
  for (const [key, callbacks] of captured) {
    const path = key.slice(0, key.indexOf("\u0000"));
    for (const cleanup of callbacks) {
      if (!current()) return;
      if (useGraphInteractionStore.getState().interactions[path] !== interactions[path]) break;
      cleanup();
    }
  }
  if (!current()) return;
  const before = useGraphInteractionStore.getState().interactions;
  const remaining = produce(before, (draft) => {
    for (const path of graphPath === undefined ? Object.keys(interactions) : [graphPath])
      if (before[path] === interactions[path]) delete draft[path];
  });
  if (remaining !== before) useGraphInteractionStore.setState({ interactions: remaining });
  if (
    current() &&
    useGraphInteractionStore.getState().interactions === remaining &&
    useGestureStore.getState() === gesture &&
    (graphPath === undefined ? Object.keys(remaining).length === 0 : !remaining[graphPath])
  )
    gesture.clearGesture(false);
}

export function resetCanvasInteractionCleanupForTests(): void {
  cleanups.clear();
}
