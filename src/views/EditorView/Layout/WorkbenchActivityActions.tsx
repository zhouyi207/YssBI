import { useTranslation } from 'react-i18next';
import { VscSettingsGear } from 'react-icons/vsc';
import type { IDockviewHeaderActionsProps } from 'dockview-react';

import { Button } from '@/components/ui/button';
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip';
import { WORKBENCH_ACTIVITY_GROUP_ID } from '@/features/core/dockview/workbenchDockviewDefaults';
import { useWorkbenchStore } from '@/features/core/workbench';

function stopHeaderControlPropagation(event: { stopPropagation(): void }): void {
  event.stopPropagation();
}

export function WorkbenchActivityActions(props: IDockviewHeaderActionsProps) {
  const { t } = useTranslation();
  const openSettings = useWorkbenchStore((state) => state.openSettings);

  if (props.group.id !== WORKBENCH_ACTIVITY_GROUP_ID || props.headerPosition !== 'left') {
    return null;
  }

  const title = t('menubar.settings');

  return (
    <div
      data-workbench-activity-actions
      className="flex h-full w-full shrink-0 items-center justify-center"
      onPointerDown={stopHeaderControlPropagation}
      onMouseDown={stopHeaderControlPropagation}
    >
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
