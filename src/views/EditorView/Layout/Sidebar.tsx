import { forwardRef, useContext, useEffect, useRef } from "react";
import { useDrag } from "../Context/DragContext";
import { useCanvas, GroupContext } from "../Context/CanvasContext";
import {
  VscEye,
  VscEyeClosed,
  VscAdd,
  VscChevronRight,
  VscChevronDown,
  VscDatabase,
  VscListUnordered
} from "react-icons/vsc";
import { useLayoutStore } from "../../../store/layoutStore";
import { useState } from "react";

const PIN_COLORS: Record<string, string> = {
  exec: "var(--exec-color)",
  int: "var(--int-color)",
  int8: "var(--int-color)",
  int16: "var(--int-color)",
  int32: "var(--int-color)",
  int64: "var(--int-color)",
  uint32: "var(--int-color)",
  uint64: "var(--int-color)",
  float: "var(--float-color)",
  float32: "var(--float-color)",
  float64: "var(--float-color)",
  bool: "var(--bool-color)",
  string: "var(--string-color)",
  date: "var(--date-color)",
  datetime: "var(--datetime-color)",
  object: "var(--object-color)",
  array: "var(--array-color)",
  struct: "#0055FF",
  delegate: "#FF3333",
  dataframe: "var(--dataframe-color)",
};

const Sidebar = forwardRef<HTMLDivElement>((_, ref) => {
  const { startDrag } = useDrag();
  const nodeId = useContext(GroupContext); // 从布局上下文获取节点 ID
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
    dataframes,
    addDataFrame,
    openSubGraph,
  } = useCanvas();

  const [expandedDataFrames, setExpandedDataFrames] = useState<Record<string, boolean>>({});

  const toggleDataFrame = (id: string, e: React.MouseEvent) => {
    e.stopPropagation();
    setExpandedDataFrames(prev => ({
      ...prev,
      [id]: !prev[id]
    }));
  };

  const listRef = useRef<HTMLDivElement>(null);

  // Read active tab from Layout Store
  const sidebarNode = useLayoutStore(s => s.nodes[nodeId || 'sidebar']);
  const activeTab = sidebarNode?.data?.currentTab as 'events' | 'functions' | 'macros' | 'variables' | 'data' | null;

  // 记录每个 Tab 的数量，用于触发滚动
  const eventsCount = Object.keys(events).length;
  const functionsCount = Object.keys(functions).length;
  const macrosCount = Object.keys(macros).length;
  const variablesCount = Object.keys(variables).length + Object.keys(globalVariables).length;
  const dataframesCount = Object.keys(dataframes || {}).length;

  // 记录上一次的数量，用于判断是否是“增加”
  const prevCounts = useRef({ events: eventsCount, functions: functionsCount, macros: macrosCount, variables: variablesCount, dataframes: dataframesCount });

  // 监听数量变化并滚动到底部
  useEffect(() => {
    const isAdded =
      eventsCount > prevCounts.current.events ||
      functionsCount > prevCounts.current.functions ||
      macrosCount > prevCounts.current.macros ||
      variablesCount > prevCounts.current.variables ||
      dataframesCount > prevCounts.current.dataframes;

    if (isAdded && listRef.current) {
      listRef.current.scrollTo({
        top: listRef.current.scrollHeight,
        behavior: 'smooth'
      });
    }

    // 更新记录
    prevCounts.current = {
      events: eventsCount,
      functions: functionsCount,
      macros: macrosCount,
      variables: variablesCount,
      dataframes: dataframesCount
    };
  }, [eventsCount, functionsCount, macrosCount, variablesCount, dataframesCount]);

  const renderItem = (id: string, name: string, type: 'variable' | 'function' | 'macro' | 'event' | 'data', extra?: any) => {
    const isSelected = selectedItemId === id && selectedItemType === type;

    return (
      <div
        key={id}
        onClick={(e) => {
          e.stopPropagation(); // 阻止事件冒泡到 Sidebar 容器
          setSelectedInfo(id, type);
        }}
        onDoubleClick={(e) => {
          if (type !== 'variable' && type !== 'data') {
            e.stopPropagation(); // 阻止事件冒泡到 Sidebar 容器
            openSubGraph(id, name, type);
          }
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
                variableType: extra?.data_type,
                variableIsArray: extra?.is_array
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
          } else if (type === 'data') {
            e.preventDefault();
            startDrag({
              type: "node-template",
              template: {
                type: "get_dataframe",
                category: "Data",
                variableId: id,
                variableName: name,
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
        {type === 'data' && (
          <button
            onClick={(e) => toggleDataFrame(id, e)}
            className="p-0.5 hover:bg-white/10 rounded text-gray-400 transition-colors"
          >
            {expandedDataFrames[id] ? <VscChevronDown size={14} /> : <VscChevronRight size={14} />}
          </button>
        )}
        <div
          className="w-2 h-2 rounded-full shrink-0"
          style={{ backgroundColor: isSelected ? 'white' : (type === 'data' ? '#10b981' : (extra?.data_type ? PIN_COLORS[extra.data_type] : '#9ca3af')) }}
        />
        <span className="flex-1 text-[12px] font-bold truncate">{name}</span>
        {type === 'data' && <VscDatabase size={12} className="opacity-40" />}
        {type === 'variable' && (
          <>
            {!extra?.isGlobal ? (
              <button
                onClick={(e) => { e.stopPropagation(); promoteVariable(id); }}
                className={`opacity-0 group-hover:opacity-100 p-1 rounded hover:bg-white/20 transition-all ${isSelected ? 'text-white' : 'text-gray-400'}`}
                title="Promote to global"
              >
                <VscEye size={12} />
              </button>
            ) : (
              <button
                onClick={(e) => { e.stopPropagation(); demoteVariable(id); }}
                className={`opacity-0 group-hover:opacity-100 p-1 rounded hover:bg-white/20 transition-all ${isSelected ? 'text-white' : 'text-gray-400'}`}
                title="Demote to local"
              >
                <VscEyeClosed size={12} />
              </button>
            )}
            <span className={`text-[9px] font-black uppercase px-1 rounded flex items-center gap-1 ${isSelected ? 'bg-white/20' : 'bg-gray-800 text-gray-500'}`}>
              {extra?.data_type}
              {extra?.is_array && <span className="text-[7px] bg-blue-500/20 text-blue-400 px-0.5 rounded">[]</span>}
            </span>
          </>
        )}
      </div>
    );
  };

  return (
    <div
      ref={ref}
      className="flex h-full w-full overflow-hidden select-none bg-[var(--sidebar-bg)]"
      onWheel={(e) => e.stopPropagation()}
    >
      <div className="flex flex-col flex-1 min-h-0 bg-[var(--sidebar-bg)]">
        {/* Header */}
        <div className="px-3 bg-[var(--workbench-bg)]/50 flex justify-between items-center h-9 shrink-0 select-none border-b border-[#2b2b2b]">
          <span className="text-[10px] font-black text-gray-500 uppercase tracking-widest">{activeTab}</span>
          <button
            onClick={(e) => {
              e.stopPropagation(); // 阻止事件冒泡到 Sidebar 容器
              if (activeTab === 'events') addEvent();
              else if (activeTab === 'functions') addFunction();
              else if (activeTab === 'macros') addMacro();
              else if (activeTab === 'variables') {
                addVariable("New Variable", "int", false);
              }
              else if (activeTab === 'data') {
                addDataFrame("New DataFrame");
              }
            }}
            className="p-1 text-gray-400 hover:text-[var(--accent-color)] transition-colors"
          >
            <VscAdd size={16} />
          </button>
        </div>

        {/* List Content */}
        <div
          ref={listRef}
          className="flex-1 overflow-y-auto p-1 custom-scrollbar scroll-smooth"
        >
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
          {activeTab === 'data' && (
            <>
              {Object.entries(dataframes).map(([id, data]) => (
                <div key={id}>
                  {renderItem(id, data.name, 'data', data)}
                  {expandedDataFrames[id] && data.columns && (
                    <div className="ml-6 mt-1 border-l border-white/10 space-y-0.5">
                      {data.columns.map((col, idx) => (
                        <div
                          key={`${id}-col-${idx}`}
                          className="flex items-center gap-2 p-1 pl-2 hover:bg-white/5 rounded cursor-grab text-[11px] text-gray-400 group/col"
                          onPointerDown={(e) => {
                            if (e.button !== 0) return;
                            e.preventDefault();
                            e.stopPropagation();
                            startDrag({
                              type: "node-template",
                              template: {
                                type: "get_column",
                                category: "Data",
                                title: `Get ${col.name}`,
                                initialData: {
                                  columnName: col.name,
                                  columnType: col.type,
                                  dataframeId: id
                                }
                              },
                              x: e.clientX, y: e.clientY, startX: e.clientX, startY: e.clientY,
                            });
                          }}
                        >
                          <VscListUnordered size={10} className="opacity-40" />
                          <span className="flex-1 truncate">{col.name}</span>
                          <span className="text-[8px] opacity-0 group-hover/col:opacity-100 transition-opacity bg-white/5 px-1 rounded uppercase">
                            {col.type.replace("Owned", "")}
                          </span>
                        </div>
                      ))}
                    </div>
                  )}
                </div>
              ))}
              {Object.keys(dataframes).length === 0 && <div className="text-[10px] text-gray-400 italic p-2 text-center">No data</div>}
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
    </div>
  );
});

export default Sidebar;
