import { useEffect, useMemo, useRef } from "react";
import { useShallow } from "zustand/react/shallow";
import { useTranslation } from "react-i18next";
import { useGraphDataStore } from "@/features/core/dataStore/graphDataStore";
import { useProjectIOStore } from "@/features/core/dataStore/projectIOStore";
import { useExecutionStore } from "@/features/core/execution/useExecutionStore";
import { useLayoutStore } from "@/features/core/layout/layoutStore";
import { getActiveLayoutTab, resolveEditorTargetGroupId } from "@/features/core/layout/layoutTabQueries";
import { getEditorGroupSelectedNodeIds } from "@/features/core/layout/editorTabStore";
import { layoutTabResourceRef } from "@/features/core/layout/layoutTabModel";
import { resolveTabDisplayName } from "@/features/application/editor/resolveTabDisplayName";
import { useSettingsStore } from "@/features/core/settings/settingsStore";
import { getViewport, subscribeToViewport, editorViewportScope, type ViewportScope } from "@/features/core/viewport";
import { LoadStatus } from "@/shared/types/ui";
import { formatDisplayPath } from "@/shared/utils/formatDisplayPath";
import {
  createBuiltInStatusBarItems,
  useStatusBarSnapshot,
  type StatusBarItemsSnapshot,
  type StatusBarRenderContext,
} from "@/features/core/statusBar";
import { useStatusBarActions } from "./useStatusBarActions";

function fileNameFromPath(path: string | null) {
  if (!path) return null;
  const displayPath = formatDisplayPath(path);
  return displayPath.replace(/\\/g, "/").split("/").pop() || displayPath;
}

function projectStatusLabel(
  status: LoadStatus,
  error: string | null,
  t: StatusBarRenderContext["t"],
) {
  if (status === LoadStatus.Error) return error ? `${t("common.error")}: ${error}` : t("bottomBar.projectError");
  if (status === LoadStatus.Loading) return t("bottomBar.loadingProject");
  if (status === LoadStatus.Ready) return t("common.ready");
  return t("common.idle");
}

function formatViewportStatus(scope: ViewportScope | null) {
  if (!scope) return "X 0 Y 0 100%";
  const viewport = getViewport(scope);
  return `X ${Math.round(viewport.x)} Y ${Math.round(viewport.y)} ${Math.round(viewport.scale * 100)}%`;
}

function ViewportStatus({ scope }: { scope: ViewportScope | null }) {
  const ref = useRef<HTMLSpanElement>(null);

  useEffect(() => {
    if (!scope) return;
    const update = () => {
      if (ref.current) ref.current.textContent = formatViewportStatus(scope);
    };
    update();
    return subscribeToViewport(scope, update);
  }, [scope?.groupId, scope?.graphPath]);

  return <span ref={ref}>{formatViewportStatus(scope)}</span>;
}

export function useStatusBarItems(): StatusBarItemsSnapshot {
  const { t } = useTranslation();
  const actions = useStatusBarActions();

  const editor = useLayoutStore(
    useShallow((state) => {
      const groupId = resolveEditorTargetGroupId(undefined, state.nodes, state);
      const active = getActiveLayoutTab(groupId, state.nodes);
      const activeTabId = active?.activeTabId ?? null;
      const activeTab = active?.tab ?? null;
      const selectedNodeIds = getEditorGroupSelectedNodeIds(groupId);

      return {
        activeEditorGroupId: groupId,
        activeTabId,
        activeTitle: activeTab
          ? resolveTabDisplayName(layoutTabResourceRef(activeTab), activeTab.id)
          : t("bottomBar.noActiveGraph"),
        activeType: activeTab?.type ?? null,
        selectedCount: Array.isArray(selectedNodeIds) ? selectedNodeIds.length : 0,
      };
    }),
  );

  const graphStats = useGraphDataStore(
    useShallow((state) => {
      if (!editor.activeTabId) return { nodeCount: 0, connectionCount: 0 };

      const nodeIds = state.getGraphNodeIds(editor.activeTabId);
      const connectionIds = new Set<string>();
      for (const nodeId of nodeIds) {
        for (const pinId of state.getGraphNodePins(editor.activeTabId, nodeId)) {
          for (const connectionId of state.getGraphPinConnections(editor.activeTabId, pinId)) {
            connectionIds.add(connectionId);
          }
        }
      }

      return {
        nodeCount: nodeIds.length,
        connectionCount: connectionIds.size,
      };
    }),
  );

  const project = useProjectIOStore(
    useShallow((state) => ({
      status: state.status,
      error: state.error,
      fileName: fileNameFromPath(state.currentPath) ?? t("bottomBar.untitledProject"),
    })),
  );

  const executionStatus = useExecutionStore((state) =>
    editor.activeTabId ? state.graphs[editor.activeTabId]?.status ?? "idle" : "idle",
  );
  const colorTheme = useSettingsStore((state) => state.appearance.colorTheme);

  const ctx = useMemo<StatusBarRenderContext>(
    () => ({
      t,
      projectStatus: projectStatusLabel(project.status, project.error, t),
      projectFileName: project.fileName,
      activeTitle: editor.activeTitle,
      activeType: editor.activeType,
      activeTabId: editor.activeTabId,
      activeEditorGroupId: editor.activeEditorGroupId,
      selectedCount: editor.selectedCount,
      nodeCount: graphStats.nodeCount,
      connectionCount: graphStats.connectionCount,
      executionStatus,
      colorTheme,
    }),
    [t, project, editor, graphStats, executionStatus, colorTheme],
  );

  const builtIn = useMemo(
    () =>
      createBuiltInStatusBarItems({
        openLogsPanel: actions.openLogsPanel,
        resetCanvasViewport: actions.resetCanvasViewport,
        cycleColorTheme: actions.cycleColorTheme,
        executionTooltip: actions.executionTooltip,
        themeTooltip: actions.themeTooltip,
        viewportTooltip: actions.viewportTooltip,
        renderViewportStatus: (groupId, graphPath) => (
          <ViewportStatus
            scope={graphPath ? editorViewportScope(groupId, graphPath) : null}
          />
        ),
      }),
    [
      actions.openLogsPanel,
      actions.resetCanvasViewport,
      actions.cycleColorTheme,
      actions.executionTooltip,
      actions.themeTooltip,
      actions.viewportTooltip,
    ],
  );

  return useStatusBarSnapshot(ctx, builtIn);
}
