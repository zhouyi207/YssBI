import { useRef, useState, useEffect } from "react";
import { PiGraph } from "react-icons/pi";
import { HiVariable } from "react-icons/hi2";
import { VscDatabase, VscTerminal } from "react-icons/vsc";
import { useLayoutStore } from "@/features/core/layout/layoutStore";

const ActivityIcon = ({ active, onClick, children, title, id }: { active: boolean; onClick: () => void; children: React.ReactNode; title: string; id: string }) => (
  <button
    onClick={onClick}
    title={title}
    data-tab-id={id}
    className={`relative w-full h-12 flex items-center justify-center transition-colors group ${active ? "text-white" : "text-[#858585] hover:text-[#cccccc]"}`}
  >
    {children}
  </button>
);

export function ActivityBar() {
  const sidebarNode = useLayoutStore((s) => s.nodes["sidebar"]);
  const activeTab = sidebarNode?.data?.currentTab as "graphs" | "variables" | "data" | "commands" | null;

  const activityBarRef = useRef<HTMLDivElement>(null);
  const [indicatorTop, setIndicatorTop] = useState({ top: 0, opacity: 0 });

  const updateNode = useLayoutStore((s) => s.updateNode);
  const previousSizeRef = useRef(260);

  const toggleTab = (tab: "graphs" | "variables" | "data" | "commands") => {
    let newTab: "graphs" | "variables" | "data" | "commands" | null = tab;
    let visible = true;

    if (activeTab === tab) {
      newTab = null;
      visible = false;
    }

    if (visible) {
      const currentSize = sidebarNode?.pixelSize || 0;
      const sizeToRestore = currentSize > 50 ? currentSize : previousSizeRef.current;

      updateNode("sidebar", {
        pixelSize: sizeToRestore,
        data: { ...sidebarNode?.data, visible: true, currentTab: newTab },
      });
    } else {
      if (sidebarNode?.pixelSize) previousSizeRef.current = sidebarNode.pixelSize;

      updateNode("sidebar", {
        data: { ...sidebarNode?.data, visible: false, currentTab: null },
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
    <div ref={activityBarRef} className="w-12 h-full bg-[#333333] flex flex-col items-center py-2 shrink-0 border-r border-[#2b2b2b] relative">
      <div
        className="absolute left-0 top-0 w-0.5 h-12 bg-[var(--accent-color)] transition-all duration-300 ease-in-out z-10 pointer-events-none"
        style={{
          transform: `translateY(${indicatorTop.top}px)`,
          opacity: indicatorTop.opacity,
        }}
      />

      <ActivityIcon id="graphs" active={activeTab === "graphs"} onClick={() => toggleTab("graphs")} title="Graphs">
        <PiGraph size={24} />
      </ActivityIcon>
      <ActivityIcon id="variables" active={activeTab === "variables"} onClick={() => toggleTab("variables")} title="Variables">
        <HiVariable size={24} />
      </ActivityIcon>
      <ActivityIcon id="data" active={activeTab === "data"} onClick={() => toggleTab("data")} title="Data">
        <VscDatabase size={24} />
      </ActivityIcon>
      <ActivityIcon id="commands" active={activeTab === "commands"} onClick={() => toggleTab("commands")} title="Commands">
        <VscTerminal size={24} />
      </ActivityIcon>
    </div>
  );
}
