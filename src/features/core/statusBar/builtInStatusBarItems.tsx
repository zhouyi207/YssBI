import {
  VscCircleFilled,
  VscFile,
  VscGitPullRequest,
  VscGraph,
  VscRadioTower,
  VscServerProcess,
  VscSymbolEvent,
  VscSymbolMethod,
  VscZoomIn,
} from "react-icons/vsc";
import { cn } from "@/lib/utils";
import type { StatusBarItemRegistration, StatusBarRenderContext } from "./statusBarItemTypes";

function typeIcon(type: string | null) {
  if (type === "event") return <VscSymbolEvent size={13} />;
  if (type === "function") return <VscSymbolMethod size={13} />;
  return <VscGraph size={13} />;
}

function executionDotClass(status: string): string {
  return cn(
    "size-2 rounded-full",
    status === "running" && "animate-pulse bg-yellow-200",
    status === "completed" && "bg-emerald-200",
    status === "error" && "bg-red-200",
    status !== "running" && status !== "completed" && status !== "error" && "bg-muted-foreground/70",
  );
}

function juliaWorkerClass(state: StatusBarRenderContext["juliaWorkerState"]): string {
  return cn(
    state === "ready" && "text-emerald-400",
    (state === "checking" || state === "starting") && "animate-pulse text-yellow-300",
    state === "unavailable" && "text-red-400",
  );
}

function executionLabel(status: string, ctx: StatusBarRenderContext): string {
  switch (status) {
    case "running":
      return ctx.t("common.running");
    case "completed":
      return ctx.t("common.completed");
    case "error":
      return ctx.t("common.error");
    default:
      return ctx.t("common.idle");
  }
}

export type BuiltInStatusBarActions = {
  openLogsPanel: () => void;
  resetCanvasViewport: () => void;
  cycleColorTheme: () => void;
  executionTooltip: string;
  themeTooltip: string;
  viewportTooltip: string;
  renderViewportStatus: (groupId: string, graphPath: string | null) => React.ReactNode;
};

export function createBuiltInStatusBarItems(actions: BuiltInStatusBarActions): StatusBarItemRegistration[] {
  return [
    {
      id: "project-status",
      alignment: "left",
      priority: 10,
      className: "bg-[var(--hover-bg)] text-foreground",
      render: (ctx) => (
        <>
          <VscGitPullRequest size={13} className="text-[var(--accent-color)]" />
          <span>{ctx.projectStatus}</span>
        </>
      ),
    },
    {
      id: "project-file",
      alignment: "left",
      priority: 20,
      className: "min-w-0",
      render: (ctx) => (
        <>
          <VscFile size={13} className="shrink-0 text-[var(--accent-color)]" />
          <span className="truncate">{ctx.projectFileName}</span>
        </>
      ),
    },
    {
      id: "active-tab",
      alignment: "left",
      priority: 30,
      className: "min-w-0 border-l border-[var(--strong-border)]",
      render: (ctx) => (
        <>
          <span className="text-[var(--accent-color)]">{typeIcon(ctx.activeType)}</span>
          <span className="truncate">{ctx.activeTitle}</span>
        </>
      ),
    },
    {
      id: "julia-worker",
      alignment: "right",
      priority: 8,
      tooltip: (ctx) => ctx.juliaWorkerTooltip,
      render: (ctx) => (
        <>
          <VscServerProcess size={13} className={juliaWorkerClass(ctx.juliaWorkerState)} />
          <span>{ctx.juliaWorkerLabel}</span>
        </>
      ),
    },
    {
      id: "node-count",
      alignment: "right",
      priority: 10,
      tooltip: (ctx) => ctx.t("bottomBar.nodeCount"),
      render: (ctx) => (
        <>
          <VscGraph size={13} className="text-[var(--accent-color)]" />
          <span>{ctx.t("bottomBar.nodes", { count: ctx.nodeCount })}</span>
        </>
      ),
    },
    {
      id: "connection-count",
      alignment: "right",
      priority: 20,
      tooltip: (ctx) => ctx.t("bottomBar.connectionCount"),
      render: (ctx) => (
        <>
          <VscRadioTower size={13} className="text-[var(--accent-color)]" />
          <span>{ctx.t("bottomBar.links", { count: ctx.connectionCount })}</span>
        </>
      ),
    },
    {
      id: "selected-nodes",
      alignment: "right",
      priority: 30,
      tooltip: (ctx) => ctx.t("bottomBar.selectedNodes"),
      render: (ctx) => (
        <>
          <VscCircleFilled
            size={9}
            className={ctx.selectedCount > 0 ? "text-[var(--accent-color)]" : "text-muted-foreground"}
          />
          <span>{ctx.t("bottomBar.selected", { count: ctx.selectedCount })}</span>
        </>
      ),
    },
    {
      id: "execution-status",
      alignment: "right",
      priority: 40,
      ariaLabel: (ctx) => actions.executionTooltip || ctx.t('bottomBar.openLogsPanel'),
      tooltip: () => actions.executionTooltip,
      onClick: () => actions.openLogsPanel(),
      render: (ctx) => (
        <>
          <span className={executionDotClass(ctx.executionStatus)} />
          <span>{executionLabel(ctx.executionStatus, ctx)}</span>
        </>
      ),
    },
    {
      id: "viewport-status",
      alignment: "right",
      priority: 50,
      ariaLabel: () => actions.viewportTooltip,
      tooltip: () => actions.viewportTooltip,
      onClick: () => actions.resetCanvasViewport(),
      render: (ctx) => (
        <>
          <VscZoomIn size={13} className="text-[var(--accent-color)]" />
          {actions.renderViewportStatus(ctx.activeEditorGroupId ?? '', ctx.activeTabId)}
        </>
      ),
    },
    {
      id: "theme-mode",
      alignment: "right",
      priority: 60,
      className: "capitalize text-foreground",
      ariaLabel: () => actions.themeTooltip,
      tooltip: () => actions.themeTooltip,
      onClick: () => actions.cycleColorTheme(),
      render: (ctx) => ctx.colorTheme,
    },
  ];
}
