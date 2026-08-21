import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { PiGraph } from "react-icons/pi";
import { HiVariable } from "react-icons/hi2";
import { VscDatabase, VscGraphLine, VscLibrary, VscSettingsGear, VscTerminal } from "react-icons/vsc";
import { useWorkbenchStore, type SidebarTabId } from '@/features/core/workbench';
import { toggleSidebarTab as persistToggleSidebarTab } from "@/features/core/layout/workbenchLayoutService";
import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { cn } from "@/lib/utils";

interface ActivityIconProps {
  active?: boolean;
  onClick: () => void;
  children: ReactNode;
  title: string;
  id: string;
  side: 'left' | 'right';
  hasPopup?: 'dialog';
}

const ActivityIcon = ({ active, onClick, children, title, id, side, hasPopup }: ActivityIconProps) => (
  <Tooltip>
    <TooltipTrigger asChild>
      <Button
        type="button"
        variant="ghost"
        size="icon"
        onClick={onClick}
        data-activity-id={id}
        data-tab-id={active === undefined ? undefined : id}
        aria-label={title}
        aria-pressed={active}
        aria-haspopup={hasPopup}
        className="size-10 bg-transparent p-0 hover:bg-transparent dark:hover:bg-transparent"
      >
        <span
          data-slot="activity-icon-surface"
          aria-hidden="true"
          className={cn(
            "flex size-9 items-center justify-center rounded-md border border-transparent transition-[color,background-color,border-color]",
            active
              ? "border-(--accent-color)/15 bg-(--accent-color)/12 text-(--accent-color)"
              : "text-muted-foreground group-hover/button:bg-(--interactive-hover) group-hover/button:text-foreground",
          )}
        >
          {children}
        </span>
      </Button>
    </TooltipTrigger>
    <TooltipContent side={side === 'left' ? 'right' : 'left'}>{title}</TooltipContent>
  </Tooltip>
);

export function ActivityBar({ side = 'left' }: { side?: 'left' | 'right' }) {
  const { t } = useTranslation();
  const sidebarCurrentTab = useWorkbenchStore((state) => state.sidebarCurrentTab);
  const sidebarHidden = useWorkbenchStore((state) => state.sidebarUserHidden);
  const openSettings = useWorkbenchStore((state) => state.openSettings);
  const activeTab = sidebarHidden ? null : sidebarCurrentTab;
  const toggleTab = (tab: SidebarTabId) => persistToggleSidebarTab(tab);
  const iconProps = (id: SidebarTabId, title: string) => ({
    id,
    title,
    side,
    active: activeTab === id,
    onClick: () => toggleTab(id),
  });

  return (
    <nav
      aria-label={t("activityBar.ariaLabel", { defaultValue: "Workbench" })}
      className={cn(
        "relative flex h-full w-11 shrink-0 flex-col items-center gap-0.5 bg-[var(--sidebar-bg)] py-2",
        side === 'right' ? 'border-l border-[var(--strong-border)]' : 'border-r border-[var(--strong-border)]',
      )}
    >
      <ActivityIcon {...iconProps("graphs", t("activityBar.graphs"))}>
        <PiGraph size={20} />
      </ActivityIcon>
      <ActivityIcon {...iconProps("nodes", t("activityBar.nodes"))}>
        <VscLibrary size={20} />
      </ActivityIcon>
      <ActivityIcon {...iconProps("variables", t("activityBar.variables"))}>
        <HiVariable size={20} />
      </ActivityIcon>
      <ActivityIcon {...iconProps("data", t("activityBar.data"))}>
        <VscDatabase size={20} />
      </ActivityIcon>
      <ActivityIcon {...iconProps("charts", t("activityBar.charts"))}>
        <VscGraphLine size={20} />
      </ActivityIcon>
      <div className="my-1 h-px w-6 bg-[var(--strong-border)]" />
      <ActivityIcon {...iconProps("commands", t("activityBar.commands"))}>
        <VscTerminal size={20} />
      </ActivityIcon>
      <div className="mt-auto">
        <ActivityIcon
          id="settings"
          title={t("menubar.settings")}
          side={side}
          hasPopup="dialog"
          onClick={openSettings}
        >
          <VscSettingsGear size={20} />
        </ActivityIcon>
      </div>
    </nav>
  );
}
