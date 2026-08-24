import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { VscDatabase, VscLibrary, VscProject, VscSettingsGear, VscTerminal } from "react-icons/vsc";
import { activateSidebarTab } from '@/features/application/editor/useSidebarTab';
import { useWorkbenchStore, type SidebarTabId } from '@/features/core/workbench';
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
        className="relative size-10 bg-transparent p-0 hover:bg-transparent dark:hover:bg-transparent"
      >
        {active ? (
          <span
            aria-hidden="true"
            className={cn(
              "pointer-events-none absolute inset-y-1.5 w-0.5 rounded-full bg-(--accent-color)",
              side === 'left' ? '-left-0.5' : '-right-0.5',
            )}
          />
        ) : null}
        <span
          data-slot="activity-icon-surface"
          aria-hidden="true"
          className={cn(
            "flex size-9 items-center justify-center rounded-md transition-[color,background-color]",
            active
              ? "bg-(--accent-color)/12 text-(--accent-color)"
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
  const openSettings = useWorkbenchStore((state) => state.openSettings);
  const activateTab = (tab: SidebarTabId) => {
    void activateSidebarTab(tab);
  };
  const iconProps = (id: SidebarTabId, title: string) => ({
    id,
    title,
    side,
    active: sidebarCurrentTab === id,
    onClick: () => activateTab(id),
  });

  return (
    <nav
      aria-label={t("activityBar.ariaLabel", { defaultValue: "Workbench" })}
      className={cn(
        "relative flex h-full w-11 shrink-0 flex-col items-center gap-0.5 bg-(--sidebar-bg) py-2",
        side === 'right' ? 'border-l border-(--strong-border)' : 'border-r border-(--strong-border)',
      )}
    >
      <ActivityIcon {...iconProps("project", t("activityBar.project"))}>
        <VscProject size={20} />
      </ActivityIcon>
      <ActivityIcon {...iconProps("nodes", t("activityBar.nodes"))}>
        <VscLibrary size={20} />
      </ActivityIcon>
      <ActivityIcon {...iconProps("data", t("activityBar.data"))}>
        <VscDatabase size={20} />
      </ActivityIcon>
      <div className="my-1 h-px w-6 bg-(--strong-border)" />
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
