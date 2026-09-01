import { workbenchDockviewRead } from "@/modules/workbench/public";
import {
  isGraphResourceDirty,
  resourceKey,
  useResourceStore,
  type ResourceRef,
} from "@/features/core/resource";

function resolveResourceDisplayName(ref: ResourceRef, fallbackId: string): string {
  const resource = useResourceStore.getState().resources[resourceKey(ref)];
  return resource?.name ?? fallbackId;
}

export interface DirtyEditorPanelSnapshot {
  /** Dockview group that owns the editor panel. */
  groupId: string;
  /** Opaque graph or chart resource reference. */
  resourceRef: string;
  /** Display title for prompts. */
  title: string;
}

/** Collect dirty editor documents once, even when a resource has multiple panels. */
export function collectDirtyEditorPanels(): DirtyEditorPanelSnapshot[] {
  const seen = new Set<string>();
  const dirty: DirtyEditorPanelSnapshot[] = [];
  for (const panel of workbenchDockviewRead.listPanels()) {
    if (panel.metadata.role !== "editor") continue;
    const { resourceKind, resourceRef } = panel.metadata;
    if (seen.has(resourceRef) || !isGraphResourceDirty(resourceRef, resourceKind)) continue;
    seen.add(resourceRef);
    dirty.push({
      groupId: panel.groupId,
      resourceRef,
      title: resolveResourceDisplayName({ id: resourceRef, kind: resourceKind }, resourceRef),
    });
  }
  return dirty;
}
