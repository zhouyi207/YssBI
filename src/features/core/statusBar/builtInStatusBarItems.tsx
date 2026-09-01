import {
  VscCircleFilled,
  VscGraph,
  VscRadioTower,
  VscServerProcess,
  VscZoomIn,
} from "react-icons/vsc";
import { cn } from "@/lib/utils";
import type { StatusBarItemRegistration, StatusBarRenderContext } from "./statusBarItemTypes";

function executionDotClass(status: string): string {
  return cn(
    "size-2 rounded-full",
    status === "running" && "animate-pulse bg-yellow-200",
    status === "completed" && "bg-emerald-200",
    status === "error" && "bg-red-200",
    status !== "running" &&
      status !== "completed" &&
      status !== "error" &&
      "bg-muted-foreground/70",
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
  executionTooltip: string;
  viewportTooltip: string;
  renderViewportStatus: (groupId: string, graphPath: string | null) => React.ReactNode;
};

export function createBuiltInStatusBarItems(
  actions: BuiltInStatusBarActions,
): StatusBarItemRegistration[] {
  return [
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
            className={
              ctx.selectedCount > 0 ? "text-[var(--accent-color)]" : "text-muted-foreground"
            }
          />
          <span>{ctx.t("bottomBar.selected", { count: ctx.selectedCount })}</span>
        </>
      ),
    },
    {
      id: "execution-status",
      alignment: "right",
      priority: 40,
      ariaLabel: (ctx) => actions.executionTooltip || ctx.t("bottomBar.openLogsPanel"),
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
          {actions.renderViewportStatus(ctx.activeEditorGroupId ?? "", ctx.activeTabId)}
        </>
      ),
    },
  ];
}
