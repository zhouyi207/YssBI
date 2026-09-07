import { VscCircleFilled, VscGraph, VscRadioTower, VscZoomIn } from "react-icons/vsc";
import type { StatusBarItemRegistration } from "./statusBarItemTypes";

export type BuiltInStatusBarActions = {
  resetCanvasViewport: () => void;
  viewportTooltip: string;
  renderViewportStatus: (groupId: string, graphPath: string | null) => React.ReactNode;
};

export function createBuiltInStatusBarItems(
  actions: BuiltInStatusBarActions,
): StatusBarItemRegistration[] {
  return [
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
      id: "viewport-status",
      alignment: "right",
      priority: 50,
      ariaLabel: () => actions.viewportTooltip,
      tooltip: () => actions.viewportTooltip,
      onClick: () => actions.resetCanvasViewport(),
      render: (ctx) => (
        <>
          <VscZoomIn size={13} className="text-[var(--accent-color)]" />
          {actions.renderViewportStatus(ctx.activeEditorGroupId ?? "", ctx.activeResourceRef)}
        </>
      ),
    },
  ];
}
