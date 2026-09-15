import { useTranslation } from "react-i18next";
import { VscSettingsGear } from "react-icons/vsc";

import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { STATUS_BAR_ICON_SIZE } from "@/shared/theme/statusBarTokens";
import { useWorkbenchUi, workbenchUi } from "../../state/ui";
import { StatusBarItem, type WorkbenchStatusBarItem } from "./StatusBarItem";

export function StatusBar({
  ariaLabel,
  left,
  right,
}: {
  readonly ariaLabel: string;
  readonly left: readonly WorkbenchStatusBarItem[];
  readonly right: readonly WorkbenchStatusBarItem[];
}) {
  return (
    <div
      data-workbench-status-bar
      className="flex h-full min-w-0 items-center justify-end overflow-hidden whitespace-nowrap text-[11px] font-medium"
      aria-label={ariaLabel}
    >
      {[...left, ...right].map((item) => (
        <StatusBarItem key={item.id} item={item} />
      ))}
    </div>
  );
}

export function WorkbenchSettingsButton() {
  const { t } = useTranslation();
  const settingsTitle = t("menubar.settings");
  const settingsOpen = useWorkbenchUi((state) => state.isSettingsOpen);

  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button
          type="button"
          className="flexlayout__border_toolbar_button"
          data-workbench-settings
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
  );
}
