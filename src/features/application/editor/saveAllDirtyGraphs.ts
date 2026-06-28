import { GraphService } from "@/services/graph/graphService";
import { useLayoutStore } from "@/features/core/layout/layoutStore";
import { useWorksheetStore } from "@/features/core/worksheet/worksheetStore";
import { collectDirtyGraphTabs } from "@/features/core/layout/tabDirty";
import { markResourceDirty } from "@/features/core/resource";
import { uiStore } from "@/features/core/ui/UIStore";
import { logger } from "@/utils/appLogger";

/**
 * Persist every dirty graph tab to disk and clear its dirty flag.
 * Returns true if all dirty tabs were saved successfully (or there were none).
 * On failure shows a toast with the offending graph and stops further saves so
 * the user can decide what to do (the remaining dirty tabs stay dirty).
 */
export async function saveAllDirtyGraphs(): Promise<boolean> {
    const dirty = collectDirtyGraphTabs();
    if (dirty.length === 0) return true;

    const layout = useLayoutStore.getState();
    for (const tab of dirty) {
        try {
            const layoutTab = Object.values(layout.nodes)
                .flatMap((node) => node.data?.tabs ?? [])
                .find((item) => item.id === tab.graphId);

            if (layoutTab?.type === 'worksheet') {
                await useWorksheetStore.getState().saveDocument(tab.graphId);
            } else if (layoutTab?.type === 'event' || layoutTab?.type === 'function') {
                await GraphService.saveProjectGraph(tab.graphId);
                markResourceDirty({ id: tab.graphId, kind: layoutTab.type }, false);
            } else {
                layout.setTabDirty(tab.graphId, false);
            }
        } catch (error) {
            const message = error instanceof Error ? error.message : String(error);
            logger.app.error(
                `Failed to save graph '${tab.title}' (${tab.graphId}): ${message}`,
                "saveAllDirtyGraphs"
            );
            uiStore.showToast(`保存「${tab.title}」失败：${message}`, "error", 3000);
            return false;
        }
    }
    return true;
}
