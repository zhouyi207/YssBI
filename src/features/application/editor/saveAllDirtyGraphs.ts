import { GraphService } from "@/services/graph/graphService";
import { useEditorTabStore } from "@/features/core/layout/editorTabStore";
import { useWorksheetStore } from "@/features/core/worksheet/worksheetStore";
import { collectDirtyGraphTabs } from "@/features/core/layout/tabDirty";
import { markResourceDirty } from "@/features/core/resource";
import { uiStore } from "@/features/core/ui/UIStore";
import { logger } from "@/utils/appLogger";
import { warnCallFunctionIssuesBeforeSave } from "@/features/application/graphDiagnostics/warnCallFunctionIssues";

/**
 * Persist every dirty graph tab to disk and clear its dirty flag.
 * Returns true if all dirty tabs were saved successfully (or there were none).
 * On failure shows a toast with the offending graph and stops further saves so
 * the user can decide what to do (the remaining dirty tabs stay dirty).
 */
export async function saveAllDirtyGraphs(): Promise<boolean> {
    const dirty = collectDirtyGraphTabs();
    if (dirty.length === 0) return true;

    const tabStore = useEditorTabStore.getState();
    for (const tab of dirty) {
        try {
            warnCallFunctionIssuesBeforeSave(tab.graphPath);
            const layoutTab = tabStore.resolveTab(tab.graphPath);

            if (layoutTab?.type === 'worksheet') {
                await useWorksheetStore.getState().saveDocument(tab.graphPath);
                markResourceDirty({ id: tab.graphPath, kind: 'worksheet' }, false);
            } else if (layoutTab?.type === 'event' || layoutTab?.type === 'function') {
                const savedPath = await GraphService.saveProjectGraph(tab.graphPath);
                markResourceDirty({ id: savedPath, kind: layoutTab.type }, false);
            }
        } catch (error) {
            const message = error instanceof Error ? error.message : String(error);
            logger.app.error(
                `Failed to save graph '${tab.title}' (${tab.graphPath}): ${message}`,
                "saveAllDirtyGraphs"
            );
            uiStore.showToast(`保存「${tab.title}」失败：${message}`, "error", 3000);
            return false;
        }
    }
    return true;
}
