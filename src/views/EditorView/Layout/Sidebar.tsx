import { forwardRef, useContext, useEffect, useRef } from "react";
import { useDraggable } from "@dnd-kit/core";
import { useEditorGroup, GroupContext } from "@/features/application/editor";
import { OverlayScrollbar } from "@/shared/ui/OverlayScrollbar";
import {
  VscEye,
  VscEyeClosed,
  VscAdd,
  VscChevronRight,
  VscChevronDown,
  VscDatabase,
  VscListUnordered,
  VscSymbolEvent,
  VscSymbolMethod,
  VscSymbolKeyword,
  VscSymbolVariable,
} from "react-icons/vsc";
import { useLayoutStore } from "@/features/core/layout/layoutStore";
import { useSidebarStore } from "@/features/core/sidebar";
import { PIN_COLORS, TYPE_ICON_COLORS, buildSidebarDragData } from "@/features/domain/sidebar";
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
 * 堆叠列表中的可折叠 section：标题栏 + 可展开内容
 * 工程化风格：无圆角、轻微背景区分、ease-out 动画
 */
const StackedCollapsibleSection = ({
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
  <div
    className={`flex flex-col shrink-0 min-h-0 ${expanded ? "flex-1" : "flex-none"}`}
    style={expanded ? { minHeight: 0 } : undefined}
  >
    <div
      role="button"
      tabIndex={0}
      onClick={(e) => {
        if ((e.target as HTMLElement).closest("[data-add-btn]")) return;
        e.stopPropagation();
        onToggle();
      }}
      onKeyDown={(e) => e.key === "Enter" && onToggle()}
      className={`group flex items-center gap-2 px-2 py-1.5 cursor-pointer shrink-0 h-7 min-h-7 transition-colors duration-150 ease-out bg-[var(--sidebar-section-bg)] text-gray-500 hover:bg-[var(--sidebar-hover)]`}
    >
      <span
        className="shrink-0 text-gray-500 transition-transform duration-150 ease-out"
        style={{ transform: expanded ? "rotate(0deg)" : "rotate(-90deg)" }}
      >
        <VscChevronDown size={12} />
      </span>
      <span className="flex-1 text-[12px] tracking-tight truncate">{label}</span>
      {onAdd && (
        <button
          data-add-btn
          type="button"
          onClick={(e) => {
            e.stopPropagation();
            onAdd();
          }}
          className="opacity-0 group-hover:opacity-100 p-0.5 hover:bg-white/[0.06] text-gray-500 transition-colors shrink-0"
        >
          <VscAdd size={11} />
        </button>
      )}
    </div>
    <div
      className="grid transition-[grid-template-rows] duration-150 ease-out overflow-hidden"
      style={{ gridTemplateRows: expanded ? "1fr" : "0fr" }}
    >
      <OverlayScrollbar className="min-h-0">
        {children}
      </OverlayScrollbar>
    </div>
  </div>
);

/**
 * 可折叠分类区块（非堆叠模式，用于嵌套结构如 Local 下的 graphName）
 * headerContent 可选，用于自定义标题（如 data 栏目的可拖拽 dataframe 行）
 */
const CollapsibleSection = ({
  label,
  expanded,
  onToggle,
  onAdd,
  headerContent,
  headerActive,
  children,
}: {
  label: string;
  expanded: boolean;
  onToggle: () => void;
  onAdd?: () => void;
  headerContent?: React.ReactNode;
  headerActive?: boolean;
  children: React.ReactNode;
}) => (
  <div className="mb-0.5">
    <div
      className={`flex items-center gap-2 py-1 pl-4 pr-2 cursor-pointer group transition-colors duration-150 ease-out ${
        headerActive ? "bg-[var(--sidebar-item-active)]" : "hover:bg-[var(--sidebar-hover)]"
      }`}
      onClick={(e) => {
        if ((e.target as HTMLElement).closest("[data-add-btn]")) return;
        if ((e.target as HTMLElement).closest("[data-draggable-header]")) return;
        e.stopPropagation();
        onToggle();
      }}
    >
      <span className="text-gray-500 shrink-0 transition-transform duration-150 ease-out" style={{ transform: expanded ? "rotate(0deg)" : "rotate(-90deg)" }}>
        <VscChevronDown size={11} />
      </span>
      {headerContent ?? <span className="flex-1 text-[12px] text-gray-500 tracking-tight">{label}</span>}
      {onAdd && (
        <button
          data-add-btn
          type="button"
          onClick={(e) => {
            e.stopPropagation();
            onAdd();
          }}
          className="opacity-0 group-hover:opacity-100 p-0.5 hover:bg-white/[0.06] text-gray-500 transition-colors shrink-0"
        >
          <VscAdd size={11} />
        </button>
      )}
    </div>
    <div
      className="grid transition-[grid-template-rows] duration-150 ease-out overflow-hidden"
      style={{ gridTemplateRows: expanded ? "1fr" : "0fr" }}
    >
      <div className="min-h-0 py-0.5 space-y-0.5">{children}</div>
    </div>
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

  const { toggleSection, isSectionExpanded } = useSidebarStore();

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
    readOnly?: boolean,
    nested?: boolean
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
          group flex items-center gap-2 pr-2 py-1.5 transition-colors duration-150 ease-out
          ${nested ? "pl-8" : "pl-4"}
          ${isSelected
            ? "bg-[var(--sidebar-item-active)] text-gray-200"
            : "hover:bg-[var(--sidebar-hover)] text-gray-400"}
        `}
      >
        <span
          className="shrink-0 flex items-center justify-center"
          style={{
            color: type === "event"
                ? TYPE_ICON_COLORS.event
                : type === "function"
                  ? TYPE_ICON_COLORS.function
                  : type === "macro"
                    ? TYPE_ICON_COLORS.macro
                    : type === "variable"
                      ? extra?.isGlobal
                        ? TYPE_ICON_COLORS.variableGlobal
                        : extra?.dataType
                          ? PIN_COLORS[typeof extra.dataType === "string" ? extra.dataType : safeDataTypeKind(extra.dataType)]
                          : TYPE_ICON_COLORS.variable
                      : type === "data"
                        ? TYPE_ICON_COLORS.data
                        : "rgba(156,163,175,0.8)",
          }}
        >
          {type === "event" && <VscSymbolEvent size={12} />}
          {type === "function" && <VscSymbolMethod size={12} />}
          {type === "macro" && <VscSymbolKeyword size={12} />}
          {type === "variable" && <VscSymbolVariable size={12} />}
          {type === "data" && <VscDatabase size={12} />}
        </span>
        <span className="flex-1 text-[12px] font-normal tracking-tight truncate">{name}</span>
        {(type === "event" || type === "function" || type === "macro") && (
          <button
            onClick={(e) => {
              e.stopPropagation();
              openGraph(id, name, type);
            }}
            className={`opacity-0 group-hover:opacity-100 p-0.5 hover:bg-white/[0.06] transition-colors ${isSelected ? "text-gray-200" : "text-gray-500"}`}
            title="Open"
          >
            <VscChevronRight size={11} />
          </button>
        )}
        {type === "variable" && !readOnly && (
          <>
            {!extra?.isGlobal ? (
              <button
                onClick={(e) => {
                  e.stopPropagation();
                  promoteVariable(id);
                }}
                className={`opacity-0 group-hover:opacity-100 p-0.5 hover:bg-white/[0.06] transition-colors ${isSelected ? "text-gray-200" : "text-gray-500"}`}
                title="Promote to global"
              >
                <VscEye size={11} />
              </button>
            ) : (
              <button
                onClick={(e) => {
                  e.stopPropagation();
                  demoteVariable(id);
                }}
                className={`opacity-0 group-hover:opacity-100 p-0.5 hover:bg-white/[0.06] transition-colors ${isSelected ? "text-gray-200" : "text-gray-500"}`}
                title="Demote to local"
              >
                <VscEyeClosed size={11} />
              </button>
            )}
            <span
              className={`text-[10px] font-normal px-1 py-0.5 flex items-center gap-1 ${isSelected ? "bg-white/[0.12] text-gray-300" : "bg-white/[0.04] text-gray-500"}`}
            >
              {safeDataTypeDisplay(extra?.dataType)}
              {extra?.dataType &&
                typeof extra.dataType === "object" &&
                "kind" in extra.dataType &&
                (extra.dataType as DataType).kind === "Array"
                  ? <span className="text-[8px] text-blue-400/80">[]</span>
                  : null}
            </span>
          </>
        )}
        {type === "variable" && readOnly && (
          <span
            className={`text-[10px] font-normal px-1 py-0.5 flex items-center gap-1 ${isSelected ? "bg-white/[0.12] text-gray-300" : "bg-white/[0.04] text-gray-500"}`}
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
        <div className="px-3 border-b border-[#2b2b2b] bg-[var(--workbench-bg)]/50 flex justify-between items-center shrink-0" style={{ height: 'var(--titlebar-height)' }}>
          <span className="text-[10px] font-black text-gray-500 uppercase tracking-widest">
            {currentTab === "graphs" ? "Graphs" : currentTab === "variables" ? "Variables" : currentTab === "data" ? "Data" : ""}
          </span>
        </div>

        <div className="flex flex-col flex-1 min-h-0 overflow-hidden p-0">
          {currentTab === "graphs" && (
            <div ref={listRef} className="flex flex-col flex-1 min-h-0">
              <StackedCollapsibleSection
                label="Event"
                expanded={isSectionExpanded("graphsEvent")}
                onToggle={() => toggleSection("graphsEvent")}
                onAdd={addEvent}
              >
                  {Object.entries(events).map(([id, data]: [string, { name: string }]) =>
                    renderItem(id, data.name, "event")
                  )}
                  {Object.keys(events).length === 0 && (
                    <div className="text-[12px] text-gray-500/70 pl-4 py-1.5">No events</div>
                  )}
              </StackedCollapsibleSection>

              <StackedCollapsibleSection
                label="Function"
                expanded={isSectionExpanded("graphsFunction")}
                onToggle={() => toggleSection("graphsFunction")}
                onAdd={addFunction}
              >
                  {Object.entries(functions).map(([id, data]: [string, { name: string }]) =>
                    renderItem(id, data.name, "function")
                  )}
                  {Object.keys(functions).length === 0 && (
                    <div className="text-[12px] text-gray-500/70 pl-4 py-1.5">No functions</div>
                  )}
              </StackedCollapsibleSection>

              <StackedCollapsibleSection
                label="Macro"
                expanded={isSectionExpanded("graphsMacro")}
                onToggle={() => toggleSection("graphsMacro")}
                onAdd={addMacro}
              >
                  {Object.entries(macros).map(([id, data]: [string, { name: string }]) =>
                    renderItem(id, data.name, "macro")
                  )}
                  {Object.keys(macros).length === 0 && (
                    <div className="text-[12px] text-gray-500/70 pl-4 py-1.5">No macros</div>
                  )}
              </StackedCollapsibleSection>

              <StackedCollapsibleSection
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
                    <div className="text-[12px] text-gray-500/70 pl-4 py-1.5">No variables</div>
                  )}
              </StackedCollapsibleSection>
            </div>
          )}

          {currentTab === "variables" && (
            <div className="flex flex-col flex-1 min-h-0">
              <StackedCollapsibleSection
                label="Global"
                expanded={isSectionExpanded("variablesGlobal")}
                onToggle={() => toggleSection("variablesGlobal")}
                onAdd={() => addVariable("New Variable", "Int32", true)}
              >
                  {Object.entries(variablesGlobal).map(([id, data]: [string, { name: string }]) =>
                    renderItem(id, data.name, "variable", { ...data, isGlobal: true }, true)
                  )}
                {Object.keys(variablesGlobal).length === 0 && (
                  <div className="text-[12px] text-gray-500/60 pl-4 py-1.5">—</div>
                )}
              </StackedCollapsibleSection>

              <StackedCollapsibleSection
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
                        renderItem(id, data.name, "variable", { ...data, isGlobal: false }, true, true)
                      )}
                    </CollapsibleSection>
                  ))}
                {localVariablesByGraph.length === 0 && (
                  <div className="text-[12px] text-gray-500/60 pl-4 py-1.5">—</div>
                )}
              </StackedCollapsibleSection>
            </div>
          )}

          {currentTab === "data" && (
            <div className="flex flex-col flex-1 min-h-0">
              <StackedCollapsibleSection
                label="Data"
                expanded={isSectionExpanded("dataData")}
                onToggle={() => toggleSection("dataData")}
                onAdd={triggerImportData}
              >
                  {Object.entries(dataframes || {}).map(([id, data]) => {
                    const name = String((data as { name?: unknown }).name ?? "");
                    const columns = (data as { columns?: Array<{ name: string; type: string }> }).columns ?? [];
                    const sectionKey = `dataData_${id}`;
                    const isSelected = selectedItemId === id && selectedItemType === "data";
                    const dragData = buildSidebarDragData(id, name, "data", data);
                    return (
                      <CollapsibleSection
                        key={id}
                        label={name}
                        expanded={isSectionExpanded(sectionKey, false)}
                        onToggle={() => toggleSection(sectionKey)}
                        headerActive={isSelected}
                        headerContent={
                          <div data-draggable-header className="flex-1 flex items-center gap-2 min-w-0">
                            <SidebarDraggableItem
                              id={id}
                              dragData={dragData}
                              onClick={(e) => {
                                e.stopPropagation();
                                toggleSection(sectionKey);
                                setSelectedInfo(id, "data");
                              }}
                              className={`group flex items-center gap-2 flex-1 min-w-0 py-0 pr-0 transition-colors duration-150 ease-out ${isSelected ? "text-gray-200" : "text-gray-400"}`}
                            >
                              <span
                                className="shrink-0 flex items-center justify-center"
                                style={{ color: TYPE_ICON_COLORS.data }}
                              >
                                <VscDatabase size={12} />
                              </span>
                              <span className="flex-1 text-[12px] font-normal tracking-tight truncate">{name}</span>
                            </SidebarDraggableItem>
                          </div>
                        }
                      >
                        {columns.map((col, idx) => (
                          <div
                            key={`${id}-col-${idx}`}
                            className="flex items-center gap-2 py-1 pl-8 pr-2 hover:bg-[var(--sidebar-hover)] text-[12px] text-gray-500 group/col transition-colors"
                          >
                            <VscListUnordered size={10} className="opacity-40 shrink-0" />
                            <span className="flex-1 truncate">{col.name}</span>
                            <span className="text-[10px] opacity-0 group-hover/col:opacity-100 transition-opacity text-gray-500 bg-white/[0.04] px-1 py-0.5">
                              {col.type.replace("Owned", "")}
                            </span>
                          </div>
                        ))}
                      </CollapsibleSection>
                    );
                  })}
                  {Object.keys(dataframes || {}).length === 0 && (
                    <div className="text-[12px] text-gray-500/70 pl-4 py-1.5">No data</div>
                  )}
              </StackedCollapsibleSection>
            </div>
          )}
        </div>
      </div>
    </div>
  );
});

export default Sidebar;
