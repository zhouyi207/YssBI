import { useGraphSessionStore } from '@/features/core/graphSession/graphSessionStore';
import { resolveEditorTargetGroupId } from '@/features/core/layout/layoutTabQueries';
import { useProjectIOStore } from '@/features/core/dataStore';
import { markResourceLoaded } from '@/features/core/resource';
import { ensureGraphViewport } from '@/features/core/viewport';
import { inferGraphResourceKind } from '@/shared/types/domain/graphResourcePath';
import { unloadGraphDocument } from './graphDocumentUnload';
import {
  enforceGraphDocumentCacheLimit,
  touchGraphDocument,
} from './graphDocumentCachePolicy';

function finishGraphEditorActivation(graphPath: string): void {
  const kind = inferGraphResourceKind(graphPath);
  if (kind) {
    markResourceLoaded({ id: graphPath, kind }, true);
  }
  ensureGraphViewport(graphPath);
}

/** Session bookkeeping + single loadGraph entry + editor activation. */
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

  const loaded = await useProjectIOStore.getState().loadGraph(graphPath);
  if (!loaded) {
    if (previous) {
      sessionStore.setFocusedSession(groupId, previous);
    } else {
      sessionStore.clearFocusedSession(groupId);
    }
    return false;
  }

  await enforceGraphDocumentCacheLimit();
  finishGraphEditorActivation(graphPath);
  return true;
}

/** Clear session only when the closed tab owned the focused graph (background tabs keep protection). */
export function deactivateGraphTab(groupId: string, closedGraphPath?: string | null): void {
  const store = useGraphSessionStore.getState();
  const focused = store.focusedSession;
  if (focused?.groupId !== groupId) return;
  if (closedGraphPath != null && focused.graphPath !== closedGraphPath) return;
  store.clearFocusedSession(groupId);
}
