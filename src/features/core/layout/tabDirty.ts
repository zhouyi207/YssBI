import type { LayoutTab } from "@/shared/types/ui";
import { useLayoutStore } from "./layoutStore";
import { isGraphResourceDirty, markResourceDirty } from "@/features/core/resource";
import { layoutTabResourceRef } from "./layoutTabModel";
import { resolveTabDisplayName } from "@/features/application/editor/resolveTabDisplayName";

export function markGraphTabDirty(graphPath: string): void {
    const located = findResourceTabLocation(graphPath);
    if (located?.tab.type === "event" || located?.tab.type === "function" || located?.tab.type === "worksheet") {
        markResourceDirty({ id: graphPath, kind: located.tab.type }, true);
        if (located.tab.pinned === false) {
            useLayoutStore.getState().setTabPinned(located.nodeId, graphPath, true);
        }
    }
}

function findResourceTabLocation(tabId: string): { nodeId: string; tab: LayoutTab } | null {
    for (const node of Object.values(useLayoutStore.getState().nodes)) {
        const tab = node.data?.tabs?.find((item) => item.id === tabId);
        if (tab) return { nodeId: node.id, tab };
    }
    return null;
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
    for (const node of Object.values(useLayoutStore.getState().nodes)) {
        if (node.type !== "component" || !node.data?.tabs) continue;
        for (const tab of node.data.tabs as LayoutTab[]) {
            if (tab.type !== "event" && tab.type !== "function" && tab.type !== "worksheet") continue;
            if (seen.has(tab.id)) continue;
            if (!isGraphResourceDirty(tab.id, tab.type)) continue;
            seen.add(tab.id);
            out.push({
              nodeId: node.id,
              graphPath: tab.id,
              title: resolveTabDisplayName(layoutTabResourceRef(tab), tab.id),
            });
        }
    }
    return out;
}
