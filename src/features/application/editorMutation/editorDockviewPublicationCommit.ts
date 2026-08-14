import {
  editorDockviewPort,
  useEditorPaneStateStore,
  type PanelInstanceId,
} from '@/features/core/dockview';
import {
  resourceKey,
  type ProjectResourceMeta,
  type ResourceKey,
  type ResourceKind,
} from '@/features/core/resource';

interface DockviewResourceMove {
  readonly from: string;
  readonly to: string;
}

function isResourceKind(kind: string): kind is ResourceKind {
  return kind === 'event'
    || kind === 'function'
    || kind === 'worksheet'
    || kind === 'database'
    || kind === 'variable';
}

export function commitEditorDockviewPublication(
  moves: Iterable<DockviewResourceMove>,
  authoritativeResources: Readonly<Record<ResourceKey, ProjectResourceMeta>>,
  commitBusinessStores: () => void,
): void | Promise<void> {
  if (!editorDockviewPort.isReady) {
    commitBusinessStores();
    return;
  }
  return commitWithDockview(moves, authoritativeResources, commitBusinessStores);
}

async function commitWithDockview(
  moves: Iterable<DockviewResourceMove>,
  authoritativeResources: Readonly<Record<ResourceKey, ProjectResourceMeta>>,
  commitBusinessStores: () => void,
): Promise<void> {
  const snapshot = await editorDockviewPort.serialize();
  const removedPanelIds: PanelInstanceId[] = [];

  try {
    for (const move of moves) await editorDockviewPort.remapResource(move.from, move.to);

    for (const panel of editorDockviewPort.listPanels()) {
      const tab = panel.tab;
      if (!tab || !isResourceKind(tab.kind)) continue;
      const key = resourceKey({ id: tab.resourceRef, kind: tab.kind });
      if (authoritativeResources[key]) continue;
      if (!await editorDockviewPort.remove(panel.panelInstanceId)) {
        throw new Error(`failed to remove editor panel '${panel.panelInstanceId}'`);
      }
      removedPanelIds.push(panel.panelInstanceId);
    }

    commitBusinessStores();
    const paneState = useEditorPaneStateStore.getState();
    for (const panelInstanceId of removedPanelIds) paneState.release(panelInstanceId);
  } catch (cause) {
    try {
      await editorDockviewPort.restore(snapshot);
    } catch (restoreCause) {
      const error = new Error(
        'editor Dockview publication failed and its layout could not be restored',
      ) as Error & { cause?: unknown; restoreCause?: unknown };
      error.cause = cause;
      error.restoreCause = restoreCause;
      throw error;
    }
    throw cause;
  }
}
