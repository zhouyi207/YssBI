import { useTranslation } from "react-i18next";
import { VscError, VscInfo, VscOutput, VscSparkle, VscTerminal } from "react-icons/vsc";

import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { useWorkbenchStatusPanelTabs } from "../../application/useWorkbenchStatusPanelTabs";

const STATUS_VIEWS = {
  problems: { Icon: VscError, titleKey: "panel.problems" },
  output: { Icon: VscOutput, titleKey: "panel.output" },
  logs: { Icon: VscTerminal, titleKey: "panel.logs" },
  details: { Icon: VscInfo, titleKey: "panel.details" },
  assistant: { Icon: VscSparkle, titleKey: "panel.assistant" },
} as const;

export function WorkbenchStatusPanelTabs({ position }: { readonly position: "bottom" | "right" }) {
  const { t } = useTranslation();
  const tabs = useWorkbenchStatusPanelTabs(position);

  return (
    <div data-workbench-status-panel-tabs={position}>
      {tabs.map(({ viewId, selected, disabled, onSelect }) => {
        const { Icon, titleKey } = STATUS_VIEWS[viewId];
        const title = t(titleKey);

        return (
          <Tooltip key={viewId}>
            <TooltipTrigger asChild>
              <button
                type="button"
                data-workbench-status-action
                data-workbench-status-panel={viewId}
                aria-label={title}
                aria-pressed={selected}
                disabled={disabled}
                onClick={onSelect}
              >
                <Icon size={16} aria-hidden />
              </button>
            </TooltipTrigger>
            <TooltipContent side="top">{title}</TooltipContent>
          </Tooltip>
        );
      })}
    </div>
  );
}
