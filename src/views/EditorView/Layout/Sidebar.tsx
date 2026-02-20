import { forwardRef, useContext, useEffect, useRef } from "react";
import { useDraggable } from "@dnd-kit/core";
import { useEditorGroup, GroupContext } from "@/features/application/editor";
import {
  VscEye,
  VscEyeClosed,
  VscAdd,
  VscChevronRight,
  VscChevronDown,
  VscDatabase,
  VscListUnordered,
} from "react-icons/vsc";
import { useLayoutStore } from "@/features/core/layout/layoutStore";
import { useSidebarStore } from "@/features/core/sidebar";
import { PIN_COLORS, buildSidebarDragData } from "@/features/domain/sidebar";
import type { DataType } from "@/shared/types/domain/dataType";
import { dataTypeDisplay } from "@/shared/types/domain/dataType";

function safeDataTypeDisplay(dt: unknown): string {
  if (typeof dt === "string") return dt;
  if (dt && typeof dt === "object" && "kind" in dt) return dataTypeDisplay(dt as DataType);
  return "";
}

function safeDataTypeKind(dt: unknown): string {
  if (dt && typeof dt === "object" && "kind" in dt) return (dt as DataType).kind;
  return "Any";
}

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

/**
 * 可折叠分类区块
 * 顶层为类别，下层为子项。点击左侧箭头展开/折叠子节点。
 */
const CollapsibleSection = ({
  label,
  expanded,
  onToggle,
  onAdd,
  children,
}: {
  label: string;
  expanded: boolean;
  onToggle: () => void;
  onAdd?: () => void;
  children: React.ReactNode;
}) => (
  <div className="mb-1">
    <div
      className="flex items-center gap-2 py-2 px-2.5 rounded-md hover:bg-white/[0.06] cursor-pointer group transition-colors"
      onClick={(e) => {
        if ((e.target as HTMLElement).closest("[data-add-btn]")) return;
        e.stopPropagation();
        onToggle();
      }}
    >
      <span className="text-gray-500 shrink-0 transition-transform duration-200" style={{ transform: expanded ? "rotate(0deg)" : "rotate(-90deg)" }}>
        <VscChevronDown size={14} />
      </span>
      <span className="flex-1 text-[11px] font-semibold text-gray-400 tracking-wide">{label}</span>
      {onAdd && (
        <button
          data-add-btn
          type="button"
          onClick={(e) => {
            e.stopPropagation();
            onAdd();
          }}
          className="opacity-0 group-hover:opacity-100 p-1 rounded hover:bg-white/10 text-gray-400 hover:text-[var(--accent-color)] transition-all shrink-0"
        >
          <VscAdd size={12} />
        </button>
      )}
    </div>
    {expanded && <div className="ml-5 pl-2 border-l border-white/[0.06] space-y-0.5">{children}</div>}
  </div>
);

const Sidebar = forwardRef<HTMLDivElement>((_, ref) => {
  useContext(GroupContext);
  const sidebarNode = useLayoutStore((s) => s.nodes["sidebar"]);
  const currentTab = sidebarNode?.data?.currentTab as "graphs" | "variables" | "data" | null;

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

  const { toggleSection, toggleDataFrame: toggleDataFrameStore, isSectionExpanded, isDataFrameExpanded } = useSidebarStore();

  const toggleDataFrame = (id: string, e: React.MouseEvent) => {
    e.stopPropagation();
    toggleDataFrameStore(id);
  };

  const listRef = useRef<HTMLDivElement>(null);

  const activeEditorNode = useLayoutStore((s) =>
    s.activeEditorGroupId ? s.nodes[s.activeEditorGroupId] : null
  );
  const activeTabId = activeEditorNode?.data?.activeTabId || null;

  // Graphs > Variable: 只显示当前选择的 graph 的 variable 和 global variable
  const { Variables: globalVariables, graphScopeVariables } = (() => {
    const global: Record<string, { name: string; dataType?: unknown }> = {};
    const local: Record<string, { name: string; dataType?: unknown }> = {};
    for (const [id, v] of Object.entries(allVariables)) {
      const scope = (v as { scope?: { type: string; eventId?: string; functionId?: string; macroId?: string } }).scope;
      if (scope?.type === "global") {
        global[id] = v as { name: string; dataType?: unknown };
      } else if (
        activeTabId &&
        scope &&
        (scope.eventId === activeTabId || scope.functionId === activeTabId || scope.macroId === activeTabId)
      ) {
        local[id] = v as { name: string; dataType?: unknown };
      }
    }
    return { Variables: global, graphScopeVariables: { ...graphVariables, ...local } };
  })();

  // Variables (read-only): Global + Local(按 graph 分组)
  const { variablesGlobal, localVariablesByGraph } = (() => {
    const global: Record<string, { name: string; dataType?: unknown }> = {};
    const byGraph: Record<string, { graphName: string; graphType: string; variables: Record<string, { name: string; dataType?: unknown }> }> = {};
    for (const [id, v] of Object.entries(allVariables)) {
      const scope = (v as { scope?: { type: string; eventId?: string; functionId?: string; macroId?: string } }).scope;
      const data = v as { name: string; dataType?: unknown };
      if (scope?.type === "global") {
        global[id] = data;
      } else {
        const graphId = scope?.eventId ?? scope?.functionId ?? scope?.macroId;
        if (graphId) {
          if (!byGraph[graphId]) {
            const meta = events[graphId] ?? functions[graphId] ?? macros[graphId];
            byGraph[graphId] = {
              graphName: (meta as { name?: string })?.name ?? graphId,
              graphType: (meta as { type?: string })?.type ?? "event",
              variables: {},
            };
          }
          byGraph[graphId].variables[id] = data;
        }
      }
    }
    const localList = Object.entries(byGraph).map(([graphId, { graphName, graphType, variables }]) => ({
      graphId,
      graphName,
      graphType,
      variables,
    }));
    return { variablesGlobal: global, localVariablesByGraph: localList };
  })();

  const eventsCount = Object.keys(events).length;
  const functionsCount = Object.keys(functions).length;
  const macrosCount = Object.keys(macros).length;
  const graphVarsCount = Object.keys(graphScopeVariables).length + Object.keys(globalVariables).length;
  const dataframesCount = Object.keys(dataframes || {}).length;

  const prevCounts = useRef({
    events: eventsCount,
    functions: functionsCount,
    macros: macrosCount,
    variables: graphVarsCount,
    dataframes: dataframesCount,
  });

  useEffect(() => {
    const isAdded =
      eventsCount > prevCounts.current.events ||
      functionsCount > prevCounts.current.functions ||
      macrosCount > prevCounts.current.macros ||
      graphVarsCount > prevCounts.current.variables ||
      dataframesCount > prevCounts.current.dataframes;

    if (isAdded && listRef.current) {
      listRef.current.scrollTo({ top: listRef.current.scrollHeight, behavior: "smooth" });
    }
    prevCounts.current = {
      events: eventsCount,
      functions: functionsCount,
      macros: macrosCount,
      variables: graphVarsCount,
      dataframes: dataframesCount,
    };
  }, [eventsCount, functionsCount, macrosCount, graphVarsCount, dataframesCount]);

  const renderItem = (
    id: string,
    name: string,
    type: "variable" | "function" | "macro" | "event" | "data",
    extra?: { dataType?: unknown; isGlobal?: boolean },
    readOnly?: boolean
  ) => {
    const isSelected = selectedItemId === id && selectedItemType === type;
    const dragData = readOnly ? null : buildSidebarDragData(id, name, type, extra as { dataType?: DataType | string } | undefined);

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
          if (type !== "variable" && type !== "data") {
            e.stopPropagation();
            openGraph(id, name, type);
          }
        }}
        className={`
          group flex items-center gap-2 px-2.5 py-2 rounded-md transition-all duration-150
          ${isSelected
            ? "bg-[var(--accent-color)]/90 text-white shadow-sm"
            : "hover:bg-white/[0.06] text-gray-300"}
        `}
      >
        {type === "data" && (
          <button
            onClick={(e) => toggleDataFrame(id, e)}
            className="p-0.5 hover:bg-white/10 rounded text-gray-400 transition-colors shrink-0"
          >
            {isDataFrameExpanded(id) ? <VscChevronDown size={12} /> : <VscChevronRight size={12} />}
          </button>
        )}
        <div
          className={`w-2.5 h-2.5 rounded-full shrink-0 ring-1 ${extra?.isGlobal ? "ring-amber-500/30" : "ring-white/10"}`}
          style={{
            backgroundColor: isSelected
              ? "white"
              : type === "data"
                ? "#10b981"
                : extra?.isGlobal
                  ? "#f59e0b"
                  : extra?.dataType
                    ? PIN_COLORS[typeof extra.dataType === "string" ? extra.dataType : safeDataTypeKind(extra.dataType)]
                    : "#9ca3af",
          }}
        />
        <span className="flex-1 text-[13px] font-medium truncate">{name}</span>
        {(type === "event" || type === "function" || type === "macro") && (
          <button
            onClick={(e) => {
              e.stopPropagation();
              openGraph(id, name, type);
            }}
            className={`opacity-0 group-hover:opacity-100 p-1 rounded-md hover:bg-white/15 transition-all ${isSelected ? "text-white" : "text-gray-400"}`}
            title="Open"
          >
            <VscChevronRight size={12} />
          </button>
        )}
        {type === "data" && <VscDatabase size={12} className="opacity-40" />}
        {type === "variable" && !readOnly && (
          <>
            {!extra?.isGlobal ? (
              <button
                onClick={(e) => {
                  e.stopPropagation();
                  promoteVariable(id);
                }}
                className={`opacity-0 group-hover:opacity-100 p-1 rounded-md hover:bg-white/15 transition-all ${isSelected ? "text-white" : "text-gray-400"}`}
                title="Promote to global"
              >
                <VscEye size={12} />
              </button>
            ) : (
              <button
                onClick={(e) => {
                  e.stopPropagation();
                  demoteVariable(id);
                }}
                className={`opacity-0 group-hover:opacity-100 p-1 rounded-md hover:bg-white/15 transition-all ${isSelected ? "text-white" : "text-gray-400"}`}
                title="Demote to local"
              >
                <VscEyeClosed size={12} />
              </button>
            )}
            <span
              className={`text-[10px] font-medium px-1.5 py-0.5 rounded flex items-center gap-1 ${isSelected ? "bg-white/25" : "bg-white/5 text-gray-500"}`}
            >
              {safeDataTypeDisplay(extra?.dataType)}
              {extra?.dataType &&
                typeof extra.dataType === "object" &&
                "kind" in extra.dataType &&
                (extra.dataType as DataType).kind === "Array" && (
                  <span className="text-[8px] text-blue-400/80">[]</span>
                )}
            </span>
          </>
        )}
        {type === "variable" && readOnly && (
          <span
            className={`text-[10px] font-medium px-1.5 py-0.5 rounded flex items-center gap-1 ${isSelected ? "bg-white/25" : "bg-white/5 text-gray-500"}`}
          >
            {safeDataTypeDisplay(extra?.dataType)}
          </span>
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
        <div className="px-4 py-3 flex items-center shrink-0 select-none border-b border-white/[0.06]">
          <span className="text-[11px] font-semibold text-gray-400 tracking-wide">
            {currentTab === "graphs" ? "Graphs" : currentTab === "variables" ? "Variables" : currentTab === "data" ? "Data" : ""}
          </span>
        </div>

        <div ref={listRef} className="flex-1 overflow-y-auto overflow-x-hidden px-2 py-2 sidebar-scrollbar scroll-smooth">
          {currentTab === "graphs" && (
            <div className="space-y-0.5">
              <CollapsibleSection
                label="Event"
                expanded={isSectionExpanded("graphsEvent")}
                onToggle={() => toggleSection("graphsEvent")}
                onAdd={addEvent}
              >
                {Object.entries(events).map(([id, data]: [string, { name: string }]) =>
                  renderItem(id, data.name, "event")
                )}
                {Object.keys(events).length === 0 && (
                  <div className="text-[12px] text-gray-500/80 italic py-3 px-2 text-center">No events</div>
                )}
              </CollapsibleSection>

              <CollapsibleSection
                label="Function"
                expanded={isSectionExpanded("graphsFunction")}
                onToggle={() => toggleSection("graphsFunction")}
                onAdd={addFunction}
              >
                {Object.entries(functions).map(([id, data]: [string, { name: string }]) =>
                  renderItem(id, data.name, "function")
                )}
                {Object.keys(functions).length === 0 && (
                  <div className="text-[12px] text-gray-500/80 italic py-3 px-2 text-center">No functions</div>
                )}
              </CollapsibleSection>

              <CollapsibleSection
                label="Macro"
                expanded={isSectionExpanded("graphsMacro")}
                onToggle={() => toggleSection("graphsMacro")}
                onAdd={addMacro}
              >
                {Object.entries(macros).map(([id, data]: [string, { name: string }]) =>
                  renderItem(id, data.name, "macro")
                )}
                {Object.keys(macros).length === 0 && (
                  <div className="text-[12px] text-gray-500/80 italic py-3 px-2 text-center">No macros</div>
                )}
              </CollapsibleSection>

              <CollapsibleSection
                label="Variable"
                expanded={isSectionExpanded("graphsVariable")}
                onToggle={() => toggleSection("graphsVariable")}
                onAdd={() => addVariable("New Variable", "Int32", false)}
              >
                {Object.keys(globalVariables).length > 0 &&
                  Object.entries(globalVariables).map(([id, data]: [string, { name: string }]) =>
                    renderItem(id, data.name, "variable", { ...data, isGlobal: true })
                  )}
                {Object.entries(graphScopeVariables).map(([id, data]: [string, { name: string }]) => {
                  if (id in globalVariables) return null;
                  return renderItem(id, data.name, "variable", { ...data, isGlobal: false });
                })}
                {Object.keys(graphScopeVariables).length === 0 && Object.keys(globalVariables).length === 0 && (
                  <div className="text-[12px] text-gray-500/80 italic py-3 px-2 text-center">No variables</div>
                )}
              </CollapsibleSection>
            </div>
          )}

          {currentTab === "variables" && (
            <div className="space-y-0.5">
              <CollapsibleSection
                label="Global"
                expanded={isSectionExpanded("variablesGlobal")}
                onToggle={() => toggleSection("variablesGlobal")}
              >
                {Object.entries(variablesGlobal).map(([id, data]: [string, { name: string }]) =>
                  renderItem(id, data.name, "variable", { ...data, isGlobal: true }, true)
                )}
                {Object.keys(variablesGlobal).length === 0 && (
                  <div className="text-[12px] text-gray-500/60 italic py-2 px-2">—</div>
                )}
              </CollapsibleSection>

              <CollapsibleSection
                label="Local"
                expanded={isSectionExpanded("variablesLocal")}
                onToggle={() => toggleSection("variablesLocal")}
              >
                {localVariablesByGraph.map(({ graphId, graphName, variables }) => (
                  <CollapsibleSection
                    key={graphId}
                    label={graphName}
                    expanded={isSectionExpanded(`variablesLocal_${graphId}`)}
                    onToggle={() => toggleSection(`variablesLocal_${graphId}`)}
                  >
                    {Object.entries(variables).map(([id, data]: [string, { name: string }]) =>
                      renderItem(id, data.name, "variable", { ...data, isGlobal: false }, true)
                    )}
                  </CollapsibleSection>
                ))}
                {localVariablesByGraph.length === 0 && (
                  <div className="text-[12px] text-gray-500/60 italic py-2 px-2">—</div>
                )}
              </CollapsibleSection>
            </div>
          )}

          {currentTab === "data" && (
            <div className="space-y-0.5">
              <CollapsibleSection
                label="Data"
                expanded={isSectionExpanded("dataData")}
                onToggle={() => toggleSection("dataData")}
                onAdd={triggerImportData}
              >
                {Object.entries(dataframes || {}).map(([id, data]) => (
                <div key={id}>
                  {renderItem(id, String((data as { name?: unknown }).name ?? ""), "data", data)}
                    {isDataFrameExpanded(id) && (data as { columns?: unknown[] }).columns && (
                      <div className="ml-5 mt-1 pl-2 border-l border-white/[0.06] space-y-0.5">
                        {((data as { columns?: Array<{ name: string; type: string }> }).columns ?? []).map(
                          (col, idx) => (
                            <div
                              key={`${id}-col-${idx}`}
                              className="flex items-center gap-2 py-1.5 px-2 rounded hover:bg-white/[0.04] text-[12px] text-gray-400 group/col transition-colors"
                            >
                              <VscListUnordered size={10} className="opacity-50 shrink-0" />
                              <span className="flex-1 truncate">{col.name}</span>
                              <span className="text-[10px] opacity-0 group-hover/col:opacity-100 transition-opacity text-gray-500 bg-white/5 px-1.5 py-0.5 rounded">
                                {col.type.replace("Owned", "")}
                              </span>
                            </div>
                          )
                        )}
                      </div>
                    )}
                </div>
              ))}
                {Object.keys(dataframes || {}).length === 0 && (
                  <div className="text-[12px] text-gray-500/80 italic py-3 px-2 text-center">No data</div>
                )}
              </CollapsibleSection>
            </div>
          )}
        </div>
      </div>
    </div>
  );
});

export default Sidebar;
