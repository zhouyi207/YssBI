import { workbenchDockviewControl } from "@/features/core/dockview/workbenchControl";
import { workbenchDockviewRead } from "@/features/core/dockview/workbenchRead";
import {
  isGraphResourceDirty,
  markResourceDirty,
  useResourceStore,
} from "@/features/core/resource";
import type { ResourceRef } from "@/features/core/resource";

export function markGraphEditorPanelDirty(graphPath: string): void {
  const panel = workbenchDockviewRead.findEditorPanelsByResource(graphPath)[0];
  if (!panel) return;

  markResourceDirty(
    {
      id: panel.metadata.resourceRef,
      kind: panel.metadata.resourceKind,
    },
    true,
  );
  if (panel.metadata.pinned === false) {
    void workbenchDockviewControl.setEditorPinned(panel.panelInstanceId, true);
  }
}

function resolveResourceDisplayName(ref: ResourceRef, fallbackId: string): string {
  const resource = useResourceStore.getState().resources[`${ref.kind}:${ref.id}`];
  return resource?.name ?? fallbackId ?? ref.id;
}

export interface DirtyEditorPanelSnapshot {
  /** Dockview group that owns the editor panel. */
  groupId: string;
  /** Graph path or worksheet resource reference. */
  graphPath: string;
  /** Display title for prompts. */
  title: string;
}

/**
 * Collect every dirty editor tab (Event/Function/Worksheet) across all editor groups,
 * deduplicated by graphPath. DocumentState is the single source of truth for dirty.
 */
export function collectDirtyEditorPanels(): DirtyEditorPanelSnapshot[] {
  const seen = new Set<string>();
  const out: DirtyEditorPanelSnapshot[] = [];
  for (const panel of workbenchDockviewRead.listPanels()) {
    if (panel.metadata.role !== "editor") continue;
    const { resourceKind, resourceRef } = panel.metadata;
    if (seen.has(resourceRef)) continue;
    if (!isGraphResourceDirty(resourceRef, resourceKind)) continue;
    seen.add(resourceRef);
    out.push({
      groupId: panel.groupId,
      graphPath: resourceRef,
      title: resolveResourceDisplayName({ id: resourceRef, kind: resourceKind }, resourceRef),
    });
  }
  return out;
}
