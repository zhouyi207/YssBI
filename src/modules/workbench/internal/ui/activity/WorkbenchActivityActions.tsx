import { useSyncExternalStore, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { VscExtensions } from "react-icons/vsc";
import type { IDockviewHeaderActionsProps } from "dockview-react";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { WORKBENCH_ACTIVITY_GROUP_ID } from "../../dockview/workbenchDockviewDefaults";
import { workbenchDockviewRead } from "../../dockview/workbenchRead";
import { revealWorkbenchView } from "../../application/workbenchLayoutActions";

type WorkbenchActivityActionsProps = IDockviewHeaderActionsProps & {
  readonly additionalActions?: ReactNode;
};

function pluginsVisible() {
  const edge = workbenchDockviewRead.getEdgeState("left");
  const activePanelId = workbenchDockviewRead
    .listGroups()
    .find((group) => group.groupId === edge.groupId)?.activePanelInstanceId;
  return (
    edge.visible &&
    !edge.collapsed &&
    workbenchDockviewRead
      .listPanels()
      .some(
        (panel) =>
          panel.metadata.role === "view" &&
          panel.metadata.viewId === "plugins" &&
          panel.panelInstanceId === activePanelId,
      )
  );
}

export function WorkbenchActivityActions({
  additionalActions,
  ...props
}: WorkbenchActivityActionsProps) {
  const { t } = useTranslation();
  const selected = useSyncExternalStore(
    workbenchDockviewRead.subscribe,
    pluginsVisible,
    () => false,
  );
  if (props.group.id !== WORKBENCH_ACTIVITY_GROUP_ID || props.headerPosition !== "left")
    return null;
  return (
    <div
      data-workbench-activity-actions
      className="flex h-auto w-full shrink-0 flex-col items-center justify-end"
      onPointerDown={(event) => event.stopPropagation()}
      onMouseDown={(event) => event.stopPropagation()}
    >
      {additionalActions}
      <Tooltip>
        <TooltipTrigger asChild>
          <button
            type="button"
            data-workbench-activity-plugins
            aria-label={t("activityBar.plugins")}
            aria-pressed={selected}
            onClick={() => void revealWorkbenchView("plugins")}
            className="flex size-10 items-center justify-center text-muted-foreground outline-none hover:text-foreground focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring"
          >
            <span
              className={
                selected
                  ? "flex size-8 items-center justify-center rounded-md bg-muted text-primary"
                  : "flex size-8 items-center justify-center rounded-md hover:bg-muted"
              }
            >
              <VscExtensions size={18} aria-hidden />
            </span>
          </button>
        </TooltipTrigger>
        <TooltipContent side="right">{t("activityBar.plugins")}</TooltipContent>
      </Tooltip>
    </div>
  );
}
