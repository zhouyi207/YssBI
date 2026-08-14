import { editorDockviewPort } from "@/features/core/dockview";
import type { LayoutTab } from "@/shared/types";
import { isGraphResourceDirty, markResourceDirty, useResourceStore } from "@/features/core/resource";
import { layoutTabResourceRef } from "./layoutTabModel";

function readLayoutTab(panel: ReturnType<typeof editorDockviewPort.listPanels>[number]): LayoutTab | null {
    const value = panel.tab?.data?.layoutTab;
    return value && typeof value === "object" ? value as LayoutTab : null;
}

export function markGraphTabDirty(graphPath: string): void {
    const panel = editorDockviewPort.findPanelsByResource(graphPath)[0];
    const tab = panel ? readLayoutTab(panel) : null;
    if (tab?.type === "event" || tab?.type === "function" || tab?.type === "worksheet") {
        markResourceDirty({ id: graphPath, kind: tab.type }, true);
        if (tab.pinned === false && panel?.tab) {
            void editorDockviewPort.updateTab(panel.panelInstanceId, {
                ...panel.tab,
                data: { ...panel.tab.data, layoutTab: { ...tab, pinned: true } },
            });
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
    for (const panel of editorDockviewPort.listPanels()) {
        const tab = readLayoutTab(panel);
        if (!tab || (tab.type !== "event" && tab.type !== "function" && tab.type !== "worksheet")) continue;
        if (seen.has(tab.id)) continue;
        if (!isGraphResourceDirty(tab.id, tab.type)) continue;
        seen.add(tab.id);
        out.push({
          nodeId: panel.groupId,
          graphPath: tab.id,
          title: resolveCoreTabDisplayName(layoutTabResourceRef(tab), tab.id),
        });
    }
    return out;
}
