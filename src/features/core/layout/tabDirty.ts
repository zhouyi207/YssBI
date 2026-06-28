import type { LayoutTab } from "@/shared/types/ui";
import { useLayoutStore } from "./layoutStore";
import { markResourceDirty } from "@/features/core/resource";

export function markGraphTabDirty(graphId: string): void {
    const tab = findResourceTab(graphId);
    if (tab?.type === "event" || tab?.type === "function") {
        markResourceDirty({ id: graphId, kind: tab.type }, true);
        return;
    }
    useLayoutStore.getState().setTabDirty(graphId, true);
}

function findResourceTab(tabId: string): LayoutTab | null {
    for (const node of Object.values(useLayoutStore.getState().nodes)) {
        const tab = node.data?.tabs?.find((item) => item.id === tabId);
        if (tab) return tab;
    }
    return null;
}

export interface DirtyTabSnapshot {
    /** Layout container that owns the tab (editor group node id). */
    nodeId: string;
    /** Graph id == tab id. */
    graphId: string;
    /** Display title for prompts. */
    title: string;
}

/**
 * Collect every dirty editor tab (Event/Function/Worksheet) across all editor groups,
 * deduplicated by graphId. Use before destructive flows (window close, project
 * switch) to ask the user once. Non-graph tabs (project picker, settings) are
 * skipped because they have no on-disk graph file to persist.
 */
export function collectDirtyGraphTabs(): DirtyTabSnapshot[] {
    const seen = new Set<string>();
    const out: DirtyTabSnapshot[] = [];
    for (const node of Object.values(useLayoutStore.getState().nodes)) {
        if (node.type !== "component" || !node.data?.tabs) continue;
        for (const tab of node.data.tabs as LayoutTab[]) {
            if (!tab.isDirty) continue;
            if (tab.type && tab.type !== "event" && tab.type !== "function" && tab.type !== "worksheet") continue;
            if (seen.has(tab.id)) continue;
            seen.add(tab.id);
            out.push({ nodeId: node.id, graphId: tab.id, title: tab.title ?? tab.id });
        }
    }
    return out;
}
