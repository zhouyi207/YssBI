import { useRef, useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { PiGraph } from "react-icons/pi";
import { HiVariable } from "react-icons/hi2";
import { VscDatabase, VscGraphLine, VscLibrary, VscTerminal } from "react-icons/vsc";
import { useLayoutStore, SIDEBAR_NODE_ID, type SidebarTabId, isSidebarTabId } from "@/features/core/layout/layoutStore";
import { WORKBENCH_CHROME_PART_ATTR } from "@/features/core/layout/workbenchSidebarDropSurface";
import { toggleSidebarTab as persistToggleSidebarTab } from "@/features/core/layout/workbenchLayoutService";
import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";

const ActivityIcon = ({ active, onClick, children, title, id }: { active: boolean; onClick: () => void; children: React.ReactNode; title: string; id: string }) => (
  <Tooltip>
    <TooltipTrigger asChild>
      <Button
        type="button"
        variant="ghost"
        onClick={onClick}
        data-tab-id={id}
        className={`relative h-12 w-full rounded-none ${active ? "text-foreground" : "text-muted-foreground hover:text-foreground"}`}
      >
        {children}
      </Button>
    </TooltipTrigger>
    <TooltipContent side="right">{title}</TooltipContent>
  </Tooltip>
);

export function ActivityBar({ side = 'left' }: { side?: 'left' | 'right' }) {
  const { t } = useTranslation();
  const sidebarNode = useLayoutStore((s) => s.nodes["sidebar"]);
  const isSidebarVisible = sidebarNode?.data?.visible !== false;
  const activeTab = isSidebarVisible && isSidebarTabId(sidebarNode?.data?.currentTab)
    ? sidebarNode.data.currentTab
    : null;

  const activityBarRef = useRef<HTMLDivElement>(null);
  const [indicatorTop, setIndicatorTop] = useState({ top: 0, opacity: 0 });

  useEffect(() => {
    const bar = activityBarRef.current;
    if (!bar || !activeTab) {
      setIndicatorTop((prev) => ({ ...prev, opacity: 0 }));
      return;
    }

    const activeEl = bar.querySelector(`[data-tab-id="${activeTab}"]`) as HTMLElement;
    if (activeEl) {
      setIndicatorTop({
        top: activeEl.offsetTop,
        opacity: 1,
      });
    } else {
      setIndicatorTop((prev) => ({ ...prev, opacity: 0 }));
    }
  }, [activeTab]);

  const toggleTab = (tab: SidebarTabId) => persistToggleSidebarTab(tab);

  return (
    <div
      ref={activityBarRef}
      {...{ [WORKBENCH_CHROME_PART_ATTR]: SIDEBAR_NODE_ID }}
      className={`w-12 h-full bg-[var(--sidebar-bg)] flex flex-col items-center py-2 shrink-0 relative ${
        side === 'right' ? 'border-l border-border' : 'border-r border-border'
      }`}
    >
      <div
        className="absolute left-0 top-0 w-0.5 h-12 bg-[var(--accent-color)] transition-all duration-300 ease-in-out z-10 pointer-events-none"
        style={{
          transform: `translateY(${indicatorTop.top}px)`,
          opacity: indicatorTop.opacity,
        }}
      />

      <ActivityIcon id="graphs" active={activeTab === "graphs"} onClick={() => toggleTab("graphs")} title={t("activityBar.graphs")}>
        <PiGraph size={24} />
      </ActivityIcon>
      <ActivityIcon id="nodes" active={activeTab === "nodes"} onClick={() => toggleTab("nodes")} title={t("activityBar.nodes")}>
        <VscLibrary size={24} />
      </ActivityIcon>
      <ActivityIcon id="variables" active={activeTab === "variables"} onClick={() => toggleTab("variables")} title={t("activityBar.variables")}>
        <HiVariable size={24} />
      </ActivityIcon>
      <ActivityIcon id="data" active={activeTab === "data"} onClick={() => toggleTab("data")} title={t("activityBar.data")}>
        <VscDatabase size={24} />
      </ActivityIcon>
      <ActivityIcon id="charts" active={activeTab === "charts"} onClick={() => toggleTab("charts")} title={t("activityBar.charts")}>
        <VscGraphLine size={24} />
      </ActivityIcon>
      <ActivityIcon id="commands" active={activeTab === "commands"} onClick={() => toggleTab("commands")} title={t("activityBar.commands")}>
        <VscTerminal size={24} />
      </ActivityIcon>
    </div>
  );
}
