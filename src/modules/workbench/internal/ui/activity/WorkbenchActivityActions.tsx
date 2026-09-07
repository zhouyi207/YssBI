import { useTranslation } from "react-i18next";
import { VscExtensions } from "react-icons/vsc";
import type { ReactNode } from "react";
import type { IDockviewHeaderActionsProps } from "dockview-react";

import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { WORKBENCH_ACTIVITY_GROUP_ID } from "../../dockview/workbenchDockviewDefaults";

function stopHeaderControlPropagation(event: { stopPropagation(): void }): void {
  event.stopPropagation();
}

type WorkbenchActivityActionsProps = IDockviewHeaderActionsProps & {
  readonly additionalActions?: ReactNode;
};

export function WorkbenchActivityActions({
  additionalActions,
  ...props
}: WorkbenchActivityActionsProps) {
  const { t } = useTranslation();

  if (props.group.id !== WORKBENCH_ACTIVITY_GROUP_ID || props.headerPosition !== "left") {
    return null;
  }

  const title = t("activityBar.plugins");

  return (
    <div
      data-workbench-activity-actions
      className="flex h-auto w-full shrink-0 flex-col items-center justify-end"
      onPointerDown={stopHeaderControlPropagation}
      onMouseDown={stopHeaderControlPropagation}
    >
      {additionalActions}
      {additionalActions ? (
        <span
          data-workbench-activity-divider
          aria-hidden="true"
          className="my-1 h-px w-6 bg-[var(--strong-border)]"
        />
      ) : null}
      <Tooltip>
        <TooltipTrigger asChild>
          <span
            role="img"
            data-workbench-activity-plugins
            aria-label={title}
            className="relative flex size-10 items-center justify-center"
          >
            <span
              aria-hidden="true"
              className="flex size-8 items-center justify-center rounded-md text-muted-foreground"
            >
              <VscExtensions size={18} />
            </span>
          </span>
        </TooltipTrigger>
        <TooltipContent side="right">{title}</TooltipContent>
      </Tooltip>
    </div>
  );
}
