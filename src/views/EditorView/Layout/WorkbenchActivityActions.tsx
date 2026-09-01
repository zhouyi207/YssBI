import { useTranslation } from "react-i18next";
import { VscSettingsGear } from "react-icons/vsc";
import type { IDockviewHeaderActionsProps } from "dockview-react";

import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { WORKBENCH_ACTIVITY_GROUP_ID } from "@/features/core/dockview/workbenchDockviewDefaults";
import { workbenchUi } from "@/features/core/workbench/ui";
import { PluginActivityActions } from "./PluginActivityActions";

function stopHeaderControlPropagation(event: { stopPropagation(): void }): void {
  event.stopPropagation();
}

export function WorkbenchActivityActions(props: IDockviewHeaderActionsProps) {
  const { t } = useTranslation();
  const openSettings = workbenchUi.openSettings;

  if (props.group.id !== WORKBENCH_ACTIVITY_GROUP_ID || props.headerPosition !== "left") {
    return null;
  }

  const title = t("menubar.settings");

  return (
    <div
      data-workbench-activity-actions
      className="flex h-auto w-full shrink-0 flex-col items-center justify-end"
      onPointerDown={stopHeaderControlPropagation}
      onMouseDown={stopHeaderControlPropagation}
    >
      <PluginActivityActions />
      <span
        data-workbench-activity-settings-divider
        aria-hidden="true"
        className="my-1 h-px w-6 bg-[var(--strong-border)]"
      />
      <Tooltip>
        <TooltipTrigger asChild>
          <Button
            type="button"
            variant="ghost"
            size="icon"
            data-workbench-activity-settings
            aria-label={title}
            aria-haspopup="dialog"
            onClick={openSettings}
            className="relative size-10 bg-transparent p-0 hover:bg-transparent dark:hover:bg-transparent"
          >
            <span
              data-workbench-activity-settings-surface
              aria-hidden="true"
              className="flex size-8 items-center justify-center rounded-md text-muted-foreground transition-[color,background-color]"
            >
              <VscSettingsGear size={18} />
            </span>
          </Button>
        </TooltipTrigger>
        <TooltipContent side="right">{title}</TooltipContent>
      </Tooltip>
    </div>
  );
}
