import { useRef, useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { PiGraph } from "react-icons/pi";
import { HiVariable } from "react-icons/hi2";
import { VscDatabase, VscGraphLine, VscLibrary, VscTerminal } from "react-icons/vsc";
import { useLayoutStore } from "@/features/core/layout/layoutStore";
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

export function ActivityBar() {
  const { t } = useTranslation();
  const sidebarNode = useLayoutStore((s) => s.nodes["sidebar"]);
  const isSidebarVisible = sidebarNode?.data?.visible !== false;
  const activeTab = isSidebarVisible
    ? (sidebarNode?.data?.currentTab as "graphs" | "nodes" | "variables" | "data" | "commands" | "charts" | null)
    : null;

  const activityBarRef = useRef<HTMLDivElement>(null);
  const [indicatorTop, setIndicatorTop] = useState({ top: 0, opacity: 0 });

  const updateNode = useLayoutStore((s) => s.updateNode);
  const previousSizeRef = useRef(260);

  const toggleTab = (tab: "graphs" | "nodes" | "variables" | "data" | "commands" | "charts") => {
    if (activeTab === tab) {
      if (sidebarNode?.pixelSize) previousSizeRef.current = sidebarNode.pixelSize;
      updateNode("sidebar", {
        data: { ...sidebarNode?.data, visible: false },
      });
    } else {
      const currentSize = sidebarNode?.pixelSize || 0;
      const sizeToRestore = currentSize > 50 ? currentSize : previousSizeRef.current;
      updateNode("sidebar", {
        pixelSize: sizeToRestore,
        data: { ...sidebarNode?.data, visible: true, currentTab: tab },
      });
    }
  };

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

  return (
    <div ref={activityBarRef} className="w-12 h-full bg-[var(--sidebar-bg)] flex flex-col items-center py-2 shrink-0 border-r border-border relative">
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
