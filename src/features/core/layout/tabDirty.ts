import { useLayoutStore } from "./layoutStore";
import { getLayoutTabById } from "./layoutTabQueries";
import { isGraphResourceDirty, markResourceDirty, useResourceStore } from "@/features/core/resource";
import { layoutTabResourceRef } from "./layoutTabModel";

import { listAllOpenEditorTabs } from "./editorTabStore";

export function markGraphTabDirty(graphPath: string): void {
    const located = getLayoutTabById(graphPath);
    if (located?.tab.type === "event" || located?.tab.type === "function" || located?.tab.type === "worksheet") {
        markResourceDirty({ id: graphPath, kind: located.tab.type }, true);
        if (located.tab.pinned === false) {
            useLayoutStore.getState().setTabPinned(located.nodeId, graphPath, true);
        }
    }
}

function resolveCoreTabDisplayName(ref: ReturnType<typeof layoutTabResourceRef>, fallbackId: string): string {
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
    for (const { groupId, tab } of listAllOpenEditorTabs()) {
        if (tab.type !== "event" && tab.type !== "function" && tab.type !== "worksheet") continue;
        if (seen.has(tab.id)) continue;
        if (!isGraphResourceDirty(tab.id, tab.type)) continue;
        seen.add(tab.id);
        out.push({
          nodeId: groupId,
          graphPath: tab.id,
          title: resolveCoreTabDisplayName(layoutTabResourceRef(tab), tab.id),
        });
    }
    return out;
}
