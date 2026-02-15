import { useRef, useState, useEffect } from "react";
import { PiGraph, PiFunction } from "react-icons/pi";
import { TiFlowSwitch } from "react-icons/ti";
import { HiVariable } from "react-icons/hi2";
import { VscDatabase } from "react-icons/vsc";
import { useLayoutStore } from "@/features/application/editor/core/stores/layoutStore";

const ActivityIcon = ({ active, onClick, children, title, id }: { active: boolean, onClick: () => void, children: React.ReactNode, title: string, id: string }) => (
    <button
        onClick={onClick}
        title={title}
        data-tab-id={id}
        className={`relative w-full h-12 flex items-center justify-center transition-colors group ${active ? 'text-white' : 'text-[#858585] hover:text-[#cccccc]'}`}
    >
        {children}
    </button>
);

export  function ActivityBar() {
    const sidebarNode = useLayoutStore(s => s.nodes['sidebar']);
    const activeTab = sidebarNode?.data?.currentTab as 'events' | 'functions' | 'macros' | 'variables' | 'data' | null;
    
    const activityBarRef = useRef<HTMLDivElement>(null);
    const [indicatorTop, setIndicatorTop] = useState({ top: 0, opacity: 0 });

    // Access layout store to control Sidebar visibility
    const updateNode = useLayoutStore(s => s.updateNode);

    // Ref to track previous size to restore
    const previousSizeRef = useRef(260);

    const toggleTab = (tab: 'events' | 'functions' | 'macros' | 'variables' | 'data') => {
        let newTab: 'events' | 'functions' | 'macros' | 'variables' | 'data' | null = tab;
        let visible = true;

        if (activeTab === tab) {
            // Toggle off
            newTab = null;
            visible = false;
        }

        // Update Sidebar Visibility and Tab
        if (visible) {
            // If becoming visible, ensure pixelSize is restored if it was 0
            const currentSize = sidebarNode?.pixelSize || 0;
            const sizeToRestore = currentSize > 50 ? currentSize : previousSizeRef.current;

            updateNode('sidebar', {
                pixelSize: sizeToRestore,
                data: { ...sidebarNode?.data, visible: true, currentTab: newTab }
            });
        } else {
            // If hiding, save current size
            if (sidebarNode?.pixelSize) previousSizeRef.current = sidebarNode.pixelSize;

            updateNode('sidebar', {
                data: { ...sidebarNode?.data, visible: false, currentTab: null }
            });
        }
    };

    // Sync active indicator
    useEffect(() => {
        const bar = activityBarRef.current;
        if (!bar || !activeTab) {
            setIndicatorTop(prev => ({ ...prev, opacity: 0 }));
            return;
        }

        const activeEl = bar.querySelector(`[data-tab-id="${activeTab}"]`) as HTMLElement;
        if (activeEl) {
            setIndicatorTop({
                top: activeEl.offsetTop,
                opacity: 1
            });
        } else {
            setIndicatorTop(prev => ({ ...prev, opacity: 0 }));
        }
    }, [activeTab]);

    return (
        <div ref={activityBarRef} className="w-12 h-full bg-[#333333] flex flex-col items-center py-2 shrink-0 border-r border-[#2b2b2b] relative">
            {/* Sliding Indicator (Vertical) */}
            <div
                className="absolute left-0 top-0 w-0.5 h-12 bg-[var(--accent-color)] transition-all duration-300 ease-in-out z-10 pointer-events-none"
                style={{
                    transform: `translateY(${indicatorTop.top}px)`,
                    opacity: indicatorTop.opacity
                }}
            />

            <ActivityIcon id="events" active={activeTab === 'events'} onClick={() => toggleTab('events')} title="Events">
                <PiGraph size={24} />
            </ActivityIcon>
            <ActivityIcon id="functions" active={activeTab === 'functions'} onClick={() => toggleTab('functions')} title="Functions">
                <PiFunction size={24} />
            </ActivityIcon>
            <ActivityIcon id="macros" active={activeTab === 'macros'} onClick={() => toggleTab('macros')} title="Macros">
                <TiFlowSwitch size={24} />
            </ActivityIcon>
            <ActivityIcon id="variables" active={activeTab === 'variables'} onClick={() => toggleTab('variables')} title="Variables">
                <HiVariable size={24} />
            </ActivityIcon>
            <ActivityIcon id="data" active={activeTab === 'data'} onClick={() => toggleTab('data')} title="Data">
                <VscDatabase size={24} />
            </ActivityIcon>
        </div>
    );
}
