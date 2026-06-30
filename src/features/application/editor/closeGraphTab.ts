import type { LayoutTab } from "@/shared/types";
import { useLayoutStore } from "@/features/core/layout/layoutStore";
import { uiStore } from "@/features/core/ui/UIStore";
import { useGraphDataStore, useProjectIOStore } from "@/features/core/dataStore";
import { GraphService } from "@/services/graph/graphService";
import { releaseGraphCacheIfClosed } from "./releaseGraphCache";
import { clearResourceDocumentState, markResourceDirty } from "@/features/core/resource";
import { syncDetailFromEditorTab } from "./syncDetailFromEditorTab";

function findTab(graphId: string): { nodeId: string; tab: LayoutTab } | null {
  for (const node of Object.values(useLayoutStore.getState().nodes)) {
    const tab = node.data?.tabs?.find((item) => item.id === graphId);
    if (tab) return { nodeId: node.id, tab };
  }
  return null;
}

function isGraphTab(tab: LayoutTab | undefined): tab is LayoutTab & { type: "event" | "function" } {
  return tab?.type === "event" || tab?.type === "function";
}

function getActiveTabAfterClose(preferredNodeId: string): LayoutTab | undefined {
  const layoutStore = useLayoutStore.getState();
  const candidateNodeIds = [
    preferredNodeId,
    layoutStore.activeEditorGroupId,
    layoutStore.activeGroupId,
  ].filter((id): id is string => Boolean(id));
  const uniqueNodeIds = Array.from(new Set(candidateNodeIds));

  for (const nodeId of uniqueNodeIds) {
    const node = layoutStore.nodes[nodeId];
    const activeTabId = node?.data?.activeTabId;
    if (!activeTabId) continue;
    const tab = node?.data?.tabs?.find((item) => item.id === activeTabId);
    if (tab) return tab;
  }

  return undefined;
}

async function restoreActiveGraphAfterClose(preferredNodeId: string): Promise<void> {
  const activeTab = getActiveTabAfterClose(preferredNodeId);
  syncDetailFromEditorTab(activeTab);

  if (!isGraphTab(activeTab)) return;
  if (useGraphDataStore.getState().graphNodes[activeTab.id] !== undefined) return;
  await useProjectIOStore.getState().loadGraph(activeTab.id);
}

export async function closeGraphTab(graphId: string, nodeId?: string, skipDirtyPrompt = false): Promise<boolean> {
  const layoutStore = useLayoutStore.getState();
  const located = nodeId
    ? { nodeId, tab: layoutStore.nodes[nodeId]?.data?.tabs?.find((tab) => tab.id === graphId) }
    : findTab(graphId);
  if (!located?.tab) return false;

  if (located.tab.isDirty && !skipDirtyPrompt) {
    const shouldSave = await uiStore.confirm({
      title: "保存更改？",
      message: `“${located.tab.title}” 已修改。关闭前是否保存？`,
      confirmText: "保存",
      cancelText: "不保存",
      type: "info",
    });
    if (shouldSave) {
      try {
        await GraphService.saveProjectGraph(graphId);
        if (located.tab.type === "event" || located.tab.type === "function") {
          markResourceDirty({ id: graphId, kind: located.tab.type }, false);
        } else {
          useLayoutStore.getState().setTabDirty(graphId, false);
        }
      } catch (error) {
        uiStore.showToast(`保存失败：${error instanceof Error ? error.message : String(error)}`, "error", 3000);
        return false;
      }
    }
  }

  useLayoutStore.getState().removeTab(located.nodeId, graphId);
  await restoreActiveGraphAfterClose(located.nodeId);
  if (located.tab.type === "event" || located.tab.type === "function") {
    clearResourceDocumentState({ id: graphId, kind: located.tab.type });
  }
  releaseGraphCacheIfClosed(graphId);
  return true;
}
