import { useEditorPaneStateStore } from '@/features/core/dockview/editorPaneStateStore';
import { workbenchDockviewInternal } from '@/features/core/dockview/workbenchDockviewInternal';
import { workbenchDockviewRead } from '@/features/core/dockview/workbenchRead';
import {
  resourceKey,
  type ProjectResourceMeta,
  type ResourceKey,
} from '@/features/core/resource';

interface DockviewResourceMove {
  readonly from: string;
  readonly to: string;
}

export function commitEditorDockviewPublication(
  moves: Iterable<DockviewResourceMove>,
  authoritativeResources: Readonly<Record<ResourceKey, ProjectResourceMeta>>,
  commitBusinessStores: () => void,
): void | Promise<void> {
  if (!workbenchDockviewRead.isReady) {
    commitBusinessStores();
    return;
  }
  return commitWithDockview(
    [...moves],
    authoritativeResources,
    commitBusinessStores,
  );
}

async function commitWithDockview(
  moves: readonly DockviewResourceMove[],
  authoritativeResources: Readonly<Record<ResourceKey, ProjectResourceMeta>>,
  commitBusinessStores: () => void,
): Promise<void> {
  const removedPanelIds = await workbenchDockviewInternal.runPublicationTransaction((transaction) => {
    for (const move of moves) transaction.remapResource(move.from, move.to);

    const removed = transaction.listPanels().flatMap((panel) => {
      if (panel.metadata.role !== 'editor') return [];
      const key = resourceKey({
        id: panel.metadata.resourceRef,
        kind: panel.metadata.resourceKind,
      });
      return authoritativeResources[key] ? [] : [panel.panelInstanceId];
    });
    transaction.removePanels(removed);
    commitBusinessStores();
    return removed;
  });

  const paneState = useEditorPaneStateStore.getState();
  for (const panelInstanceId of removedPanelIds) paneState.release(panelInstanceId);
}
