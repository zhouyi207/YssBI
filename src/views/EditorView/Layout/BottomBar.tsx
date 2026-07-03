import { useEffect, useRef } from "react";
import { useShallow } from "zustand/react/shallow";
import { useTranslation } from "react-i18next";
import {
  VscCircleFilled,
  VscFile,
  VscGitPullRequest,
  VscGraph,
  VscRadioTower,
  VscSymbolEvent,
  VscSymbolMethod,
  VscZoomIn,
} from "react-icons/vsc";
import { useGraphDataStore } from "@/features/core/dataStore/graphDataStore";
import { useProjectIOStore } from "@/features/core/dataStore/projectIOStore";
import { useExecutionStore } from "@/features/core/execution/useExecutionStore";
import { useLayoutStore } from "@/features/core/layout/layoutStore";
import { getActiveLayoutTab } from "@/features/core/layout/layoutTabQueries";
import { useSettingsStore } from "@/features/core/settings/settingsStore";
import { getViewport, subscribeToViewport } from "@/features/core/viewport";
import { LoadStatus } from "@/shared/types/ui";
import { cn } from "@/lib/utils";
import { formatDisplayPath } from "@/shared/utils/formatDisplayPath";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";

function fileNameFromPath(path: string | null) {
  if (!path) return null;
  const displayPath = formatDisplayPath(path);
  return displayPath.replace(/\\/g, "/").split("/").pop() || displayPath;
}

function executionLabel(status: string, t: (key: string) => string) {
  switch (status) {
    case "running":
      return t("common.running");
    case "completed":
      return t("common.completed");
    case "error":
      return t("common.error");
    default:
      return t("common.idle");
  }
}

function projectStatusLabel(status: LoadStatus, error: string | null, t: (key: string) => string) {
  if (status === LoadStatus.Error) return error ? `${t("common.error")}: ${error}` : t("bottomBar.projectError");
  if (status === LoadStatus.Loading) return t("bottomBar.loadingProject");
  if (status === LoadStatus.Ready) return t("common.ready");
  return t("common.idle");
}

const StatusItem = ({
  children,
  className,
  tooltip,
  ...props
}: React.HTMLAttributes<HTMLDivElement> & { tooltip?: string }) => {
  const item = (
    <div
      className={cn(
        "flex h-full items-center gap-1.5 px-2 text-muted-foreground transition-colors hover:bg-[var(--hover-bg)] hover:text-foreground",
        className,
      )}
      {...props}
    >
      {children}
    </div>
  );

  if (!tooltip) return item;

  return (
    <Tooltip>
      <TooltipTrigger asChild>{item}</TooltipTrigger>
      <TooltipContent side="top">{tooltip}</TooltipContent>
    </Tooltip>
  );
};

function formatViewportStatus(graphId: string | null) {
  if (!graphId) return `X 0 Y 0 100%`;
  const viewport = getViewport(graphId);
  return `X ${Math.round(viewport.x)} Y ${Math.round(viewport.y)} ${Math.round(viewport.scale * 100)}%`;
}

function ViewportStatus({ graphId }: { graphId: string | null }) {
  const ref = useRef<HTMLSpanElement>(null);

  useEffect(() => {
    if (!graphId) return;
    const update = () => {
      if (ref.current) ref.current.textContent = formatViewportStatus(graphId);
    };

    update();
    return subscribeToViewport(graphId, update);
  }, [graphId]);

  return <span ref={ref}>{formatViewportStatus(graphId)}</span>;
}

export function BottomBar() {
  const { t } = useTranslation();
  const editor = useLayoutStore(
    useShallow((state) => {
      const groupId = state.activeEditorGroupId ?? state.activeGroupId ?? "default_editor";
      const active = getActiveLayoutTab(groupId, state.nodes);
      const activeTabId = active?.activeTabId ?? null;
      const activeTab = active?.tab ?? null;
      const selectedNodeIds = node?.data?.params?.selectedNodeIds;

      return {
        groupId,
        activeTabId,
        activeTitle: activeTab?.title ?? t("bottomBar.noActiveGraph"),
        activeType: activeTab?.type ?? null,
        selectedCount: Array.isArray(selectedNodeIds) ? selectedNodeIds.length : 0,
      };
    }),
  );

  const graphStats = useGraphDataStore(
    useShallow((state) => {
      if (!editor.activeTabId) return { nodeCount: 0, connectionCount: 0 };

      const nodeIds = state.graphNodes[editor.activeTabId] ?? [];
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
  const themeMode = useSettingsStore((state) => state.theme.mode);

  const typeIcon =
    editor.activeType === "event" ? <VscSymbolEvent size={13} /> :
    editor.activeType === "function" ? <VscSymbolMethod size={13} /> :
    <VscGraph size={13} />;

  return (
    <footer className="flex h-6 shrink-0 items-center justify-between overflow-hidden border-t border-[var(--strong-border)] bg-[var(--panel-bg)] text-[11px] font-medium text-[var(--panel-fg)] shadow-[0_-1px_0_rgba(0,0,0,0.06)]">
      <div className="flex h-full min-w-0 items-center">
        <StatusItem className="bg-[var(--hover-bg)] text-foreground">
          <VscGitPullRequest size={13} className="text-[var(--accent-color)]" />
          <span>{projectStatusLabel(project.status, project.error, t)}</span>
        </StatusItem>
        <StatusItem className="min-w-0">
          <VscFile size={13} className="shrink-0 text-[var(--accent-color)]" />
          <span className="truncate">{project.fileName}</span>
        </StatusItem>
        <StatusItem className="min-w-0 border-l border-[var(--strong-border)]">
          <span className="text-[var(--accent-color)]">{typeIcon}</span>
          <span className="truncate">{editor.activeTitle}</span>
        </StatusItem>
      </div>

      <div className="flex h-full shrink-0 items-center">
        <StatusItem tooltip={t("bottomBar.nodeCount")}>
          <VscGraph size={13} className="text-[var(--accent-color)]" />
          <span>{t("bottomBar.nodes", { count: graphStats.nodeCount })}</span>
        </StatusItem>
        <StatusItem tooltip={t("bottomBar.connectionCount")}>
          <VscRadioTower size={13} className="text-[var(--accent-color)]" />
          <span>{t("bottomBar.links", { count: graphStats.connectionCount })}</span>
        </StatusItem>
        <StatusItem tooltip={t("bottomBar.selectedNodes")}>
          <VscCircleFilled size={9} className={editor.selectedCount > 0 ? "text-[var(--accent-color)]" : "text-muted-foreground"} />
          <span>{t("bottomBar.selected", { count: editor.selectedCount })}</span>
        </StatusItem>
        <StatusItem tooltip={t("bottomBar.executionStatus")}>
          <span
            className={cn(
              "size-2 rounded-full",
              executionStatus === "running" && "animate-pulse bg-yellow-200",
              executionStatus === "completed" && "bg-emerald-200",
              executionStatus === "error" && "bg-red-200",
              executionStatus !== "running" &&
                executionStatus !== "completed" &&
                executionStatus !== "error" &&
                "bg-muted-foreground/70",
            )}
          />
          <span>{executionLabel(executionStatus, t)}</span>
        </StatusItem>
        <StatusItem tooltip={t("bottomBar.canvasViewport")}>
          <VscZoomIn size={13} className="text-[var(--accent-color)]" />
          <ViewportStatus graphId={editor.activeTabId} />
        </StatusItem>
        <StatusItem tooltip={t("bottomBar.themeMode")} className="capitalize text-foreground">
          {themeMode}
        </StatusItem>
      </div>
    </footer>
  );
}
