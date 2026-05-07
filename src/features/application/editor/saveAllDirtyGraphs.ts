import { GraphService } from "@/services/graph/graphService";
import { useLayoutStore } from "@/features/core/layout/layoutStore";
import { collectDirtyGraphTabs } from "@/features/core/layout/tabDirty";
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
            await GraphService.saveProjectGraph(tab.graphId);
            layout.setTabDirty(tab.graphId, false);
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
