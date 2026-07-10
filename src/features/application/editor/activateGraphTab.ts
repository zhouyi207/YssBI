import { useGraphSessionStore } from '@/features/core/graphSession/graphSessionStore';
import { resolveEditorTargetGroupId } from '@/features/core/layout/layoutTabQueries';
import { useProjectIOStore } from '@/features/core/dataStore';
import { unloadGraphDocument } from './graphSessionLifecycle';
import { activateCachedGraph, isGraphCachedInMemory } from './graphLoadPolicy';
import {
  enforceGraphDocumentCacheLimit,
  touchGraphDocument,
} from './graphDocumentCachePolicy';

/** Session bookkeeping + backend load for a graph path in a group. */
export async function activateGraphTab(
  graphPath: string,
  targetGroupId?: string,
): Promise<boolean> {
  const groupId = resolveEditorTargetGroupId(targetGroupId);
  const sessionStore = useGraphSessionStore.getState();
  const previous = sessionStore.setFocusedSession(groupId, graphPath);

  if (previous && previous !== graphPath) {
    await unloadGraphDocument(previous);
  }

  touchGraphDocument(graphPath);

  if (isGraphCachedInMemory(graphPath)) {
    await enforceGraphDocumentCacheLimit();
    return activateCachedGraph(graphPath);
  }

  const loaded = await useProjectIOStore.getState().loadGraph(graphPath);
  if (loaded) {
    touchGraphDocument(graphPath);
    await enforceGraphDocumentCacheLimit();
    return true;
  }

  if (previous) {
    sessionStore.setFocusedSession(groupId, previous);
  } else {
    sessionStore.clearFocusedSession(groupId);
  }
  return false;
}

export function deactivateGraphTab(groupId: string): void {
  useGraphSessionStore.getState().clearFocusedSession(groupId);
}
