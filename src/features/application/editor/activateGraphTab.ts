import { useGraphSessionStore } from '@/features/core/graphSession/graphSessionStore';
import { resolveEditorTargetGroupId } from '@/features/core/layout/layoutTabQueries';
import { useProjectIOStore } from '@/features/core/dataStore';
import { deactivateInactiveGraphPath } from './deactivateInactiveGraphPath';
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
  const previous = sessionStore.setGroupActivePath(groupId, graphPath);

  if (previous && previous !== graphPath) {
    await deactivateInactiveGraphPath(previous);
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
    useGraphSessionStore.getState().setGroupActivePath(groupId, previous);
  } else {
    useGraphSessionStore.getState().clearGroupActivePath(groupId);
  }
  return false;
}

export function deactivateGraphTab(groupId: string): void {
  useGraphSessionStore.getState().clearGroupActivePath(groupId);
}
