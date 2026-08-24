import { layoutTabFromEditorMetadata } from '@/features/core/dockview/workbenchPanelModel';
import { workbenchDockviewPort } from '@/features/core/dockview/workbenchDockviewPort';
import { isGraphResourceDirty, markResourceDirty, useResourceStore } from '@/features/core/resource';
import { layoutTabResourceRef } from './layoutTabModel';

export function markGraphTabDirty(graphPath: string): void {
  const panel = workbenchDockviewPort.findEditorPanelsByResource(graphPath)[0];
  if (!panel || panel.metadata.role !== 'editor') return;

  const tab = layoutTabFromEditorMetadata(panel.metadata);
  markResourceDirty({
    id: panel.metadata.resourceRef,
    kind: panel.metadata.resourceKind,
  }, true);
  if (tab.pinned === false) {
    void workbenchDockviewPort.setEditorPinned(panel.panelInstanceId, true);
  }
}

function resolveCoreTabDisplayName(
  ref: ReturnType<typeof layoutTabResourceRef>,
  fallbackId: string,
): string {
  if (!ref) return fallbackId || 'Untitled';
  const resource = useResourceStore.getState().resources[`${ref.kind}:${ref.id}`];
  return resource?.name ?? fallbackId ?? ref.id;
}

export interface DirtyTabSnapshot {
    /** Layout container that owns the tab (editor group node id). */
    nodeId: string;
    /** Graph path or worksheet id == tab id. */
    graphPath: string;
    /** Display title for prompts. */
    title: string;
}

/**
 * Collect every dirty editor tab (Event/Function/Worksheet) across all editor groups,
 * deduplicated by graphPath. DocumentState is the single source of truth for dirty.
 */
export function collectDirtyGraphTabs(): DirtyTabSnapshot[] {
  const seen = new Set<string>();
  const out: DirtyTabSnapshot[] = [];
  for (const panel of workbenchDockviewPort.listPanels()) {
    if (panel.metadata.role !== 'editor') continue;
    const tab = layoutTabFromEditorMetadata(panel.metadata);
    if (seen.has(tab.id)) continue;
    if (!isGraphResourceDirty(tab.id, panel.metadata.resourceKind)) continue;
    seen.add(tab.id);
    out.push({
      nodeId: panel.groupId,
      graphPath: tab.id,
      title: resolveCoreTabDisplayName(layoutTabResourceRef(tab), tab.id),
    });
  }
  return out;
}
