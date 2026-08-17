import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { PiGraph } from "react-icons/pi";
import { HiVariable } from "react-icons/hi2";
import { VscDatabase, VscGraphLine, VscLibrary, VscTerminal } from "react-icons/vsc";
import { useWorkbenchStore, type SidebarTabId } from '@/features/core/workbench';
import { toggleSidebarTab as persistToggleSidebarTab } from "@/features/core/layout/workbenchLayoutService";
import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { cn } from "@/lib/utils";

interface ActivityIconProps {
  active: boolean;
  onClick: () => void;
  children: ReactNode;
  title: string;
  id: string;
  side: 'left' | 'right';
}

const ActivityIcon = ({ active, onClick, children, title, id, side }: ActivityIconProps) => (
  <Tooltip>
    <TooltipTrigger asChild>
      <Button
        type="button"
        variant="ghost"
        onClick={onClick}
        data-tab-id={id}
        aria-label={title}
        aria-pressed={active}
        className={cn(
          "relative size-10 rounded-md border border-transparent transition-[color,background-color,border-color]",
          active
            ? "border-[var(--accent-color)]/15 bg-[var(--accent-color)]/12 text-[var(--accent-color)] shadow-sm"
            : "text-muted-foreground hover:bg-[var(--interactive-hover)] hover:text-foreground",
        )}
      >
        {children}
        {active ? (
          <span
            aria-hidden="true"
            className={cn(
              "absolute top-1/2 h-5 w-0.5 -translate-y-1/2 rounded-full bg-[var(--accent-color)]",
              side === 'left' ? "-left-1" : "-right-1",
            )}
          />
        ) : null}
      </Button>
    </TooltipTrigger>
    <TooltipContent side={side === 'left' ? 'right' : 'left'}>{title}</TooltipContent>
  </Tooltip>
);

export function ActivityBar({ side = 'left' }: { side?: 'left' | 'right' }) {
  const { t } = useTranslation();
  const sidebarCurrentTab = useWorkbenchStore((state) => state.sidebarCurrentTab);
  const sidebarHidden = useWorkbenchStore((state) => state.sidebarUserHidden);
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
    </nav>
  );
}
