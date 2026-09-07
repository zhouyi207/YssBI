import { useTranslation } from "react-i18next";
import { VscSettingsGear } from "react-icons/vsc";

import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { STATUS_BAR_ICON_SIZE } from "@/shared/theme/statusBarTokens";
import { useWorkbenchUi, workbenchUi } from "../../state/ui";
import { StatusBarItem, type WorkbenchStatusBarItem } from "./StatusBarItem";
import { WorkbenchStatusPanelTabs } from "./WorkbenchStatusPanelTabs";

export function StatusBar({
  ariaLabel,
  left,
  right,
}: {
  readonly ariaLabel: string;
  readonly left: readonly WorkbenchStatusBarItem[];
  readonly right: readonly WorkbenchStatusBarItem[];
}) {
  const { t } = useTranslation();
  const settingsTitle = t("menubar.settings");
  const settingsOpen = useWorkbenchUi((state) => state.isSettingsOpen);

  return (
    <footer
      data-workbench-status-bar
      className="relative flex h-(--statusbar-height) shrink-0 items-center justify-between overflow-hidden border-t border-(--strong-border) bg-(--panel-header-bg) text-[11px] font-medium text-foreground"
      aria-label={ariaLabel}
    >
      <div className="absolute inset-y-0 left-0 flex w-11 items-center justify-center">
        <Tooltip>
          <TooltipTrigger asChild>
            <button
              type="button"
              data-workbench-status-action
              data-workbench-status-settings
              aria-label={settingsTitle}
              aria-haspopup="dialog"
              aria-expanded={settingsOpen}
              onClick={workbenchUi.openSettings}
            >
              <VscSettingsGear size={STATUS_BAR_ICON_SIZE} aria-hidden />
            </button>
          </TooltipTrigger>
          <TooltipContent side="top">{settingsTitle}</TooltipContent>
        </Tooltip>
      </div>
      <div
        className="flex h-full shrink-0 items-center"
        style={{ paddingLeft: "max(var(--workbench-center-offset, 0px), 44px)" }}
      >
        <WorkbenchStatusPanelTabs position="bottom" />
        {left.map((item) => (
          <StatusBarItem key={item.id} item={item} />
        ))}
      </div>
      <div
        className="flex h-full min-w-0 items-center justify-end overflow-hidden"
        style={{ marginRight: "max(var(--workbench-center-right-offset, 0px), 76px)" }}
      >
        {right.map((item) => (
          <StatusBarItem key={item.id} item={item} />
        ))}
      </div>
      <div className="absolute inset-y-0 right-0 flex items-center">
        <WorkbenchStatusPanelTabs position="right" />
      </div>
    </footer>
  );
}
