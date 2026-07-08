import { useProjectIOStore } from '@/features/core/dataStore';
import { resolveEditorTargetGroupId } from '@/features/core/layout/layoutTabQueries';
import { useGraphSessionStore } from '@/features/core/graphSession/graphSessionStore';
import { deactivateInactiveGraphPath } from './deactivateInactiveGraphPath';

/** Session bookkeeping + backend reload for a graph path in a group. */
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
  const loaded = await useProjectIOStore.getState().loadGraph(graphPath);
  if (loaded) return true;

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
