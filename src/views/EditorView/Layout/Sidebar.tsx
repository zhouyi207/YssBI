import { forwardRef, useContext, useEffect, useRef, useState } from "react";
import { useDraggable } from "@dnd-kit/core";
import { useEditorGroup, GroupContext } from "@/features/application/editor";
import {
  VscEye,
  VscEyeClosed,
  VscAdd,
  VscChevronRight,
  VscChevronDown,
  VscDatabase,
  VscListUnordered
} from "react-icons/vsc";
import { useLayoutStore } from "@/features/core/layout/layoutStore";
import { PIN_COLORS, buildSidebarDragData, buildColumnDragData } from "@/features/domain/sidebar";
import { dataTypeKind, dataTypeDisplay } from "@/shared/types/domain/dataType";

/**
 * 可拖拽的侧边栏项 — 整行可拖拽。
 * 必须定义在 Sidebar 组件外以保证 useDraggable hook 稳定。
 *
 * PointerSensor activationConstraint (distance: 5) 确保：
 *  - 简单 click / doubleClick 不会触发拖拽
 *  - 按住并移动 >= 5px 后才开始拖拽
 */
const SidebarDraggableItem: React.FC<{
  id: string;
  dragData: { type: string; template?: unknown } | null;
  children: React.ReactNode;
  className?: string;
  onClick?: (e: React.MouseEvent) => void;
  onDoubleClick?: (e: React.MouseEvent) => void;
}> = ({ id, dragData, children, className, onClick, onDoubleClick }) => {
  const canDrag = !!dragData;
  const { attributes, listeners, setNodeRef, isDragging } = useDraggable({
    id: `sidebar-item-${id}`,
    data: dragData ?? { type: "node-template", template: {} },
    disabled: !canDrag,
  });

  return (
    <div
      ref={setNodeRef}
      {...(canDrag ? listeners : {})}
      {...(canDrag ? attributes : {})}
      onClick={onClick}
      onDoubleClick={onDoubleClick}
      className={`${className ?? ""} ${canDrag ? "cursor-grab active:cursor-grabbing" : ""}`}
      style={{
        opacity: isDragging ? 0.5 : 1,
        touchAction: canDrag ? "none" : undefined,
      }}
    >
      {children}
    </div>
  );
};

const Sidebar = forwardRef<HTMLDivElement>((_, ref) => {
  const nodeId = useContext(GroupContext) as string | null; // 从布局上下文获取节点 ID
  const {
    variables: graphVariables,
    Variables: allVariables,
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
    triggerImportData,
    openGraph,
  } = useEditorGroup();

  const [expandedDataFrames, setExpandedDataFrames] = useState<Record<string, boolean>>({});

  const toggleDataFrame = (id: string, e: React.MouseEvent) => {
    e.stopPropagation();
    setExpandedDataFrames(prev => ({
      ...prev,
      [id]: !prev[id]
    }));
  };

  const listRef = useRef<HTMLDivElement>(null);

  // 当前激活的图 ID（用于过滤局部变量）
  const activeEditorNode = useLayoutStore((s) =>
    s.activeEditorGroupId ? s.nodes[s.activeEditorGroupId] : null
  );
  const activeTabId = activeEditorNode?.data?.activeTabId || null;

  // 按 scope 分区：Global = scope.type==='global'，Local = 当前图的 event/function/macro 变量
  const { Variables: globalVariables, localVariables } = (() => {
    const global: Record<string, { name: string; dataType?: unknown }> = {};
    const local: Record<string, { name: string; dataType?: unknown }> = {};
    for (const [id, v] of Object.entries(allVariables)) {
      const scope = (v as { scope?: { type: string; eventId?: string; functionId?: string; macroId?: string } }).scope;
      if (scope?.type === 'global') {
        global[id] = v as { name: string; dataType?: unknown };
      } else if (activeTabId && scope && (scope.eventId === activeTabId || scope.functionId === activeTabId || scope.macroId === activeTabId)) {
        local[id] = v as { name: string; dataType?: unknown };
      }
    }
    return { Variables: global, localVariables: { ...graphVariables, ...local } };
  })();

  // Read active tab from Layout Store
  const sidebarNode = useLayoutStore(s => s.nodes[nodeId ?? 'sidebar']);
  const activeTab = sidebarNode?.data?.currentTab as 'events' | 'functions' | 'macros' | 'variables' | 'data' | null;

  // 记录每个 Tab 的数量，用于触发滚动
  const eventsCount = Object.keys(events).length;
  const functionsCount = Object.keys(functions).length;
  const macrosCount = Object.keys(macros).length;
  const variablesCount = Object.keys(localVariables).length + Object.keys(globalVariables).length;
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
    const dragData = buildSidebarDragData(id, name, type, extra);

    return (
      <SidebarDraggableItem
        key={id}
        id={id}
        dragData={dragData}
        onClick={(e) => {
          e.stopPropagation();
          setSelectedInfo(id, type);
        }}
        onDoubleClick={(e) => {
          if (type !== 'variable' && type !== 'data') {
            e.stopPropagation();
            openGraph(id, name, type);
          }
        }}
        className={`
          group flex items-center gap-1 p-1.5 rounded transition-all border
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
          style={{ backgroundColor: isSelected ? 'white' : (type === 'data' ? '#10b981' : (extra?.dataType ? PIN_COLORS[typeof extra.dataType === 'string' ? extra.dataType : dataTypeKind(extra.dataType)] : '#9ca3af')) }}
        />
        <span className="flex-1 text-[12px] font-bold truncate">{name}</span>
        {/* 为 event/function/macro 添加打开按钮 */}
        {(type === 'event' || type === 'function' || type === 'macro') && (
          <button
            onClick={(e) => {
              e.stopPropagation();
              openGraph(id, name, type);
            }}
            className={`opacity-0 group-hover:opacity-100 p-1 rounded hover:bg-white/20 transition-all ${isSelected ? 'text-white' : 'text-gray-400'}`}
            title="Open"
          >
            <VscChevronRight size={12} />
          </button>
        )}
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
              {extra?.dataType ? (typeof extra.dataType === 'string' ? extra.dataType : dataTypeDisplay(extra.dataType)) : ''}
              {extra?.dataType && typeof extra.dataType === 'object' && 'kind' in extra.dataType && extra.dataType.kind === 'Array' && (
                <span className="text-[7px] bg-blue-500/20 text-blue-400 px-0.5 rounded">[]</span>
              )}
            </span>
          </>
        )}
      </SidebarDraggableItem>
    );
  };

  return (
    <div
      ref={ref}
      className="sidebar-container flex h-full w-full overflow-hidden select-none bg-[var(--sidebar-bg)] relative z-30"
      style={{ pointerEvents: "auto" }}
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
                addVariable("New Variable", "Int32", false);
              }
              else if (activeTab === 'data') {
                triggerImportData();
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
              {Object.entries(events).map(([id, data]: [string, { name: string }]) => renderItem(id, data.name, 'event'))}
              {Object.keys(events).length === 0 && <div className="text-[10px] text-gray-400 italic p-2 text-center">No events</div>}
            </>
          )}
          {activeTab === 'functions' && (
            <>
              {Object.entries(functions).map(([id, data]: [string, { name: string }]) => renderItem(id, data.name, 'function'))}
              {Object.keys(functions).length === 0 && <div className="text-[10px] text-gray-400 italic p-2 text-center">No functions</div>}
            </>
          )}
          {activeTab === 'macros' && (
            <>
              {Object.entries(macros).map(([id, data]: [string, { name: string }]) => renderItem(id, data.name, 'macro'))}
              {Object.keys(macros).length === 0 && <div className="text-[10px] text-gray-400 italic p-2 text-center">No macros</div>}
            </>
          )}
          {activeTab === 'data' && (
            <>
              {Object.entries(dataframes).map(([id, data]) => (
                <div key={id}>
                  {renderItem(id, String((data as { name?: unknown }).name ?? ''), 'data', data)}
                  {expandedDataFrames[id] && (data as { columns?: unknown[] }).columns && (
                    <div className="ml-6 mt-1 border-l border-white/10 space-y-0.5">
                      {((data as { columns?: Array<{ name: string; type: string }> }).columns ?? []).map((col, idx) => {
                        const columnDragData = buildColumnDragData(id, idx, col);

                        return (
                          <SidebarDraggableItem
                            key={`${id}-col-${idx}`}
                            id={`${id}-col-${idx}`}
                            dragData={columnDragData}
                            className="flex items-center gap-2 p-1 pl-2 hover:bg-white/5 rounded cursor-grab text-[11px] text-gray-400 group/col"
                          >
                            <VscListUnordered size={10} className="opacity-40" />
                            <span className="flex-1 truncate">{col.name}</span>
                            <span className="text-[8px] opacity-0 group-hover/col:opacity-100 transition-opacity bg-white/5 px-1 rounded uppercase">
                              {col.type.replace("Owned", "")}
                            </span>
                          </SidebarDraggableItem>
                        );
                      })}
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
                  {Object.entries(globalVariables).map(([id, data]: [string, { name: string }]) => renderItem(id, data.name, 'variable', { ...data, isGlobal: true }))}
                </div>
              )}
              {/* Local */}
              <div className="mb-2">
                {Object.keys(localVariables).length > 0 && (
                  <div className="px-2 py-1 text-[8px] font-black text-gray-400 uppercase tracking-tighter flex items-center gap-2">
                    Local
                    <div className="h-px flex-1 bg-white/5" />
                  </div>
                )}
                {Object.entries(localVariables).map(([id, data]: [string, { name: string }]) => renderItem(id, data.name, 'variable', { ...data, isGlobal: false }))}
              </div>
              {Object.keys(localVariables).length === 0 && Object.keys(globalVariables).length === 0 && (
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
