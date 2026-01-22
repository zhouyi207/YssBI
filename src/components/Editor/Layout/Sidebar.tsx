import { useState, useEffect, useRef } from "react";
import { useDrag } from "../Context/DragContext";
import { useCanvas } from "../Context/CanvasContext";
import { useUI } from "../Context/UIProvider";
import {
  VscEye,
  VscEyeClosed,
  VscAdd
} from "react-icons/vsc";
import { PiGraph, PiFunction } from "react-icons/pi";
import { TiFlowSwitch } from "react-icons/ti";
import { HiVariable } from "react-icons/hi2";


const PIN_COLORS: Record<string, string> = {
  exec: "var(--exec-color)",
  int: "var(--int-color)",
  float: "var(--float-color)",
  bool: "var(--bool-color)",
  string: "var(--string-color)",
  object: "var(--object-color)",
  array: "#ef4444",
  struct: "#f97316",
  delegate: "#ec4899",
};

export default function Sidebar() {
  const { startDrag } = useDrag();
  const {
    variables,
    globalVariables,
    selectedItemId,
    selectedItemType,
    setSelectedInfo,
    addVariable,
    promoteVariable,
    demoteVariable,
    functions,
    addFunction,
    macros,
    addMacro,
    events,
    addEvent,
    openSubGraph,
    tabs,
    groups,
    closeGroup,
    activeGroupId
  } = useCanvas();

  const { showDialog } = useUI();

  // Activity Bar & Sidebar View State
  const [activeTab, setActiveTab] = useState<'events' | 'functions' | 'macros' | 'variables' | null>('variables');
  const activityBarRef = useRef<HTMLDivElement>(null);
  const [indicatorTop, setIndicatorTop] = useState({ top: 0, opacity: 0 });

  const toggleTab = (tab: 'events' | 'functions' | 'macros' | 'variables') => {
    if (activeTab === tab) {
      setActiveTab(null); // Collapse
    } else {
      setActiveTab(tab);
    }
  };

  // Auto-collapse/close empty groups in split view
  useEffect(() => {
    if (tabs.length === 0 && groups.length > 1) {
      closeGroup(activeGroupId);
    }
  }, [tabs.length, groups.length, activeGroupId, closeGroup]);

  // Update vertical sliding indicator
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

  // Helper to get unique name
  const getUniqueName = (baseName: string, items: Record<string, { name: string }>) => {
    const names = Object.values(items).map(i => i.name);
    let name = baseName;
    let counter = 1;
    while (names.includes(name)) {
      name = `${baseName}_${counter}`;
      counter++;
    }
    return name;
  };

  const renderItem = (id: string, name: string, type: 'variable' | 'function' | 'macro' | 'event', extra?: any) => {
    const isSelected = selectedItemId === id && selectedItemType === type;

    return (
      <div
        key={id}
        onClick={() => {
          setSelectedInfo(id, type);
        }}
        onDoubleClick={() => {
          if (type !== 'variable') openSubGraph(id, name, type);
        }}
        onPointerDown={(e) => {
          if (e.button !== 0) return;
          if ((e.target as HTMLElement).closest('button')) return;
          if (type === 'variable') {
            e.preventDefault();
            startDrag({
              type: "node-template",
              template: {
                type: "get_variable",
                category: "Variable",
                variableId: id,
                variableName: name,
                variableType: extra?.type
              },
              x: e.clientX, y: e.clientY, startX: e.clientX, startY: e.clientY,
            });
          } else if (type === 'function' || type === 'macro') {
            e.preventDefault();
            startDrag({
              type: "node-template",
              template: {
                type: `call_${type}`,
                category: type === 'function' ? "Functions" : "Macros",
                subGraphId: id,
                subName: name,
              },
              x: e.clientX, y: e.clientY, startX: e.clientX, startY: e.clientY,
            });
          }
        }}
        className={`
          group flex items-center gap-2 p-1.5 rounded cursor-grab transition-all border
          ${isSelected
            ? 'bg-[var(--accent-color)] text-white border-[var(--accent-color)] shadow-sm'
            : 'hover:bg-white/5 text-gray-300 border-transparent'}
        `}
      >
        <div
          className="w-2 h-2 rounded-full shrink-0"
          style={{ backgroundColor: isSelected ? 'white' : (extra?.type ? PIN_COLORS[extra.type] : '#9ca3af') }}
        />
        <span className="flex-1 text-[12px] font-bold truncate">{name}</span>

        {type === 'variable' && (
          <>
            {!extra?.isGlobal ? (
              <button
                onClick={(e) => { e.stopPropagation(); promoteVariable(id); }}
                className={`opacity-0 group-hover:opacity-100 p-1 rounded hover:bg-white/20 transition-all ${isSelected ? 'text-white' : 'text-gray-400'}`}
                title="提升为全局变量"
              >
                <VscEye size={12} />
              </button>
            ) : (
              <button
                onClick={(e) => { e.stopPropagation(); demoteVariable(id); }}
                className={`opacity-0 group-hover:opacity-100 p-1 rounded hover:bg-white/20 transition-all ${isSelected ? 'text-white' : 'text-gray-400'}`}
                title="降级为局部变量"
              >
                <VscEyeClosed size={12} />
              </button>
            )}
            <span className={`text-[9px] font-black uppercase px-1 rounded ${isSelected ? 'bg-white/20' : 'bg-gray-800 text-gray-500'}`}>
              {extra?.type}
            </span>
          </>
        )}
      </div>
    );
  };

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

  return (
    <div className="flex h-full overflow-hidden select-none bg-[var(--sidebar-bg)] border-r border-[#2b2b2b]" onWheel={(e) => e.stopPropagation()}>
      {/* Activity Bar */}
      <div ref={activityBarRef} className="relative w-12 bg-[#333333] flex flex-col items-center py-2 shrink-0 border-r border-[#2b2b2b]">
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
      </div>

      {/* Sidebar Content Panel */}
      {activeTab && (
        <div className="w-52 flex flex-col min-h-0 bg-[var(--sidebar-bg)]">
          {/* Header */}
          <div className="px-3 bg-[var(--workbench-bg)]/50 flex justify-between items-center h-9 shrink-0 select-none border-b border-[#2b2b2b]">
            <span className="text-[10px] font-black text-gray-500 uppercase tracking-widest">{activeTab}</span>
            <button
              onClick={() => {
                if (activeTab === 'events') addEvent(getUniqueName("NewEvent", events));
                else if (activeTab === 'functions') addFunction(getUniqueName("NewFunction", functions));
                else if (activeTab === 'macros') addMacro(getUniqueName("NewMacro", macros));
                else if (activeTab === 'variables') {
                  const allVars = { ...variables, ...globalVariables };
                  addVariable(getUniqueName("NewVar", allVars), "int", false);
                }
              }}
              className="p-1 text-gray-400 hover:text-[var(--accent-color)] transition-colors"
            >
              <VscAdd size={16} />
            </button>
          </div>

          {/* List Content */}
          <div className="flex-1 overflow-y-auto p-1 custom-scrollbar">
            {activeTab === 'events' && (
              <>
                {Object.entries(events).map(([id, data]) => renderItem(id, data.name, 'event'))}
                {Object.keys(events).length === 0 && <div className="text-[10px] text-gray-400 italic p-2 text-center">No events</div>}
              </>
            )}
            {activeTab === 'functions' && (
              <>
                {Object.entries(functions).map(([id, data]) => renderItem(id, data.name, 'function'))}
                {Object.keys(functions).length === 0 && <div className="text-[10px] text-gray-400 italic p-2 text-center">No functions</div>}
              </>
            )}
            {activeTab === 'macros' && (
              <>
                {Object.entries(macros).map(([id, data]) => renderItem(id, data.name, 'macro'))}
                {Object.keys(macros).length === 0 && <div className="text-[10px] text-gray-400 italic p-2 text-center">No macros</div>}
              </>
            )}
            {activeTab === 'variables' && (
              <>
                {/* Global */}
                {Object.keys(globalVariables).length > 0 && (
                  <div className="mb-2">
                    <div className="px-2 py-1 text-[8px] font-black text-gray-400 uppercase tracking-tighter flex items-center gap-2">
                      Global
                      <div className="h-px flex-1 bg-white/5" />
                    </div>
                    {Object.entries(globalVariables).map(([id, data]) => renderItem(id, data.name, 'variable', { ...data, isGlobal: true }))}
                  </div>
                )}
                {/* Local */}
                <div className="mb-2">
                  {Object.keys(globalVariables).length > 0 && (
                    <div className="px-2 py-1 text-[8px] font-black text-gray-400 uppercase tracking-tighter flex items-center gap-2">
                      Local
                      <div className="h-px flex-1 bg-white/5" />
                    </div>
                  )}
                  {Object.entries(variables).map(([id, data]) => renderItem(id, data.name, 'variable', { ...data, isGlobal: false }))}
                </div>
                {Object.keys(variables).length === 0 && Object.keys(globalVariables).length === 0 && (
                  <div className="text-[10px] text-gray-400 italic p-2 text-center">No variables</div>
                )}
              </>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
