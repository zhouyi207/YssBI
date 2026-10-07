import { VscCircleFilled, VscGraph, VscRadioTower, VscZoomIn } from "react-icons/vsc";
import { STATUS_BAR_ICON_SIZE } from "@/shared/theme/statusBarTokens";
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
      visible: (ctx) => ctx.activeResourceRef !== null,
      tooltip: (ctx) => ctx.t("bottomBar.nodeCount"),
      render: (ctx) => (
        <>
          <VscGraph size={STATUS_BAR_ICON_SIZE} className="shrink-0 text-[var(--accent-color)]" />
          <span>{ctx.t("bottomBar.nodes", { count: ctx.nodeCount })}</span>
        </>
      ),
    },
    {
      id: "connection-count",
      alignment: "right",
      priority: 20,
      visible: (ctx) => ctx.activeResourceRef !== null,
      tooltip: (ctx) => ctx.t("bottomBar.connectionCount"),
      render: (ctx) => (
        <>
          <VscRadioTower
            size={STATUS_BAR_ICON_SIZE}
            className="shrink-0 text-[var(--accent-color)]"
          />
          <span>{ctx.t("bottomBar.links", { count: ctx.connectionCount })}</span>
        </>
      ),
    },
    {
      id: "selected-nodes",
      alignment: "right",
      priority: 30,
      visible: (ctx) => ctx.activeResourceRef !== null,
      tooltip: (ctx) => ctx.t("bottomBar.selectedNodes"),
      render: (ctx) => (
        <>
          <VscCircleFilled
            size={STATUS_BAR_ICON_SIZE}
            className={
              ctx.selectedCount > 0
                ? "shrink-0 text-[var(--accent-color)]"
                : "shrink-0 text-muted-foreground"
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
      visible: (ctx) => ctx.activeResourceRef !== null,
      ariaLabel: () => actions.viewportTooltip,
      tooltip: () => actions.viewportTooltip,
      onClick: () => actions.resetCanvasViewport(),
      render: (ctx) => (
        <>
          <VscZoomIn size={STATUS_BAR_ICON_SIZE} className="shrink-0 text-[var(--accent-color)]" />
          {actions.renderViewportStatus(ctx.activeEditorGroupId ?? "", ctx.activeResourceRef)}
        </>
      ),
    },
  ];
}
