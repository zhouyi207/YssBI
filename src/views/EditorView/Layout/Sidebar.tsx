import { forwardRef, useCallback, useContext, useEffect, useId, useRef } from "react";
import { useTranslation } from "react-i18next";
import { useDraggable, useDroppable } from "@dnd-kit/core";
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
  VscSymbolVariable,
  VscDiscard,
  VscRedo,
  VscFolder,
} from "react-icons/vsc";
import { useLayoutStore } from "@/features/core/layout/layoutStore";
import { useHistoryStore } from "@/features/core/history";
import type { HistoryEntry } from "@/features/core/history";
import { useSidebarStore } from "@/features/core/sidebar";
import { buildSidebarDragData } from "@/features/application/sidebar";
import { DROP_TYPES, DRAG_TYPES } from "@/features/core/dnd";
import { TYPE_ICON_COLORS } from "@/features/domain/sidebar";
import type { DataType } from "@/shared/types/domain/dataType";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { ContextMenu } from "@/shared/ui/contextMenu";
import { GraphService } from "@/services/graph/graphService";
import { useGraphMetaStore, useProjectIOStore } from "@/features/core/dataStore";
import { openDataViewWindow, safeDataTypeColor, safeDataTypeDisplay } from "./sidebarUtils";
import {
  buildSidebarContextMenuSections,
  useSidebarContextMenu,
  type GraphResourceType,
} from "./sidebarContextMenu";

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
  style?: React.CSSProperties;
  onClick?: (e: React.MouseEvent) => void;
  onDoubleClick?: (e: React.MouseEvent) => void;
  onContextMenu?: (e: React.MouseEvent) => void;
}> = ({ id, dragData, children, className, style, onClick, onDoubleClick, onContextMenu }) => {
  const canDrag = !!dragData;
  const { attributes, listeners, setNodeRef } = useDraggable({
    id: `sidebar-item-${id}`,
    data: dragData ?? { type: DRAG_TYPES.NODE_TEMPLATE, template: {} },
    disabled: !canDrag,
  });

  return (
    <div
      ref={setNodeRef}
      {...(canDrag ? listeners : {})}
      {...(canDrag ? attributes : {})}
      onClick={onClick}
      onDoubleClick={onDoubleClick}
      onContextMenu={onContextMenu}
      className={`${className ?? ""} ${canDrag ? "cursor-grab active:cursor-grabbing" : ""}`}
      style={{
        ...style,
        opacity: 1,
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
  dropTarget,
  onHeaderContextMenu,
  onContentContextMenu,
  children,
}: {
  label: string;
  expanded: boolean;
  onToggle: () => void;
  onAdd?: () => void;
  dropTarget?: GraphFolderDropTarget;
  onHeaderContextMenu?: (e: React.MouseEvent) => void;
  onContentContextMenu?: (e: React.MouseEvent) => void;
  children: React.ReactNode;
}) => {
  const fallbackDropId = useId();
  const { setNodeRef, isOver } = useDroppable({
    id: dropTarget
      ? `graph-folder-drop-${dropTarget.graphType}-${dropTarget.folderPath || "root"}`
      : `graph-folder-drop-disabled-${fallbackDropId}`,
    data: dropTarget
      ? { dropType: DROP_TYPES.GRAPH_FOLDER, graphType: dropTarget.graphType, folderPath: dropTarget.folderPath }
      : undefined,
    disabled: !dropTarget,
  });

  return (
    <div
      ref={setNodeRef}
      className={`flex flex-col shrink-0 min-h-0 ${expanded ? "flex-1" : "flex-none"} ${
        isOver ? "bg-[var(--sidebar-hover)]" : ""
      }`}
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
        onKeyDown={(e) => {
          if (e.key !== "Enter" && e.key !== " ") return;
          e.preventDefault();
          onToggle();
        }}
        onContextMenu={onHeaderContextMenu}
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
          <Button
            data-add-btn
            type="button"
            variant="ghost"
            size="icon-xs"
            onClick={(e) => {
              e.stopPropagation();
              onAdd();
            }}
            className="shrink-0 text-gray-500 opacity-0 transition-opacity group-hover:opacity-100"
          >
            <VscAdd size={11} />
          </Button>
        )}
      </div>
      <div
        className="grid transition-[grid-template-rows] duration-150 ease-out overflow-hidden"
        style={{ gridTemplateRows: expanded ? "1fr" : "0fr" }}
      >
        <OverlayScrollbar className="min-h-0 flex-1">
          <div className="min-h-full" onContextMenu={onContentContextMenu}>
            {children}
          </div>
        </OverlayScrollbar>
      </div>
    </div>
  );
};

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
  headerContentToggles = true,
  headerActive,
  indentDepth = 0,
  dropTarget,
  onContextMenu,
  children,
}: {
  label: string;
  expanded: boolean;
  onToggle: () => void;
  onAdd?: () => void;
  headerContent?: React.ReactNode;
  headerContentToggles?: boolean;
  headerActive?: boolean;
  indentDepth?: number;
  dropTarget?: GraphFolderDropTarget;
  onContextMenu?: (e: React.MouseEvent) => void;
  children: React.ReactNode;
}) => {
  const fallbackDropId = useId();
  const { setNodeRef, isOver } = useDroppable({
    id: dropTarget
      ? `graph-folder-drop-${dropTarget.graphType}-${dropTarget.folderPath || "root"}`
      : `graph-folder-drop-disabled-${fallbackDropId}`,
    data: dropTarget
      ? { dropType: DROP_TYPES.GRAPH_FOLDER, graphType: dropTarget.graphType, folderPath: dropTarget.folderPath }
      : undefined,
    disabled: !dropTarget,
  });

  return (
    <div ref={setNodeRef} className={isOver ? "bg-[var(--sidebar-hover)]" : undefined}>
      <div
        role="button"
        tabIndex={0}
        className={`flex items-center gap-2 py-1 pr-2 cursor-pointer group transition-colors duration-150 ease-out ${
          headerActive ? "bg-[var(--sidebar-item-active)]" : "hover:bg-[var(--sidebar-hover)]"
        }`}
        style={{ paddingLeft: 16 + indentDepth * 16 }}
        onClick={(e) => {
          if ((e.target as HTMLElement).closest("[data-add-btn]")) return;
          if (!headerContentToggles && (e.target as HTMLElement).closest("[data-header-content]")) return;
          e.stopPropagation();
          onToggle();
        }}
        onKeyDown={(e) => {
          if (e.key !== "Enter" && e.key !== " ") return;
          e.preventDefault();
          onToggle();
        }}
        onContextMenu={onContextMenu}
      >
        <span className="text-gray-500 shrink-0 transition-transform duration-150 ease-out" style={{ transform: expanded ? "rotate(0deg)" : "rotate(-90deg)" }}>
          <VscChevronDown size={11} />
        </span>
        {headerContent ? (
          <div data-header-content className="flex-1 min-w-0">
            {headerContent}
          </div>
        ) : (
          <span className="flex-1 text-[12px] text-gray-500 tracking-tight">{label}</span>
        )}
        {onAdd && (
          <Button
            data-add-btn
            type="button"
            variant="ghost"
            size="icon-xs"
            onClick={(e) => {
              e.stopPropagation();
              onAdd();
            }}
            className="shrink-0 text-gray-500 opacity-0 transition-opacity group-hover:opacity-100"
          >
            <VscAdd size={11} />
          </Button>
        )}
      </div>
      <div
        className="grid transition-[grid-template-rows] duration-150 ease-out overflow-hidden"
        style={{ gridTemplateRows: expanded ? "1fr" : "0fr" }}
      >
        <div className="min-h-0">{children}</div>
      </div>
    </div>
  );
};

interface GraphFolderDropTarget {
  graphType: GraphResourceType;
  folderPath: string;
}

interface GraphTreeNode {
  name: string;
  path: string;
  folders: Map<string, GraphTreeNode>;
  graphs: Array<[string, { name: string; folderPath?: string }]>;
}

function createGraphTreeNode(name = "", path = ""): GraphTreeNode {
  return { name, path, folders: new Map(), graphs: [] };
}

function joinFolderPath(parent: string, name: string): string {
  const cleanName = name.trim();
  if (!cleanName) return parent;
  return parent ? `${parent}/${cleanName}` : cleanName;
}

function ensureFolder(root: GraphTreeNode, folderPath: string): GraphTreeNode {
  const parts = folderPath.split("/").map((part) => part.trim()).filter(Boolean);
  let node = root;
  let current = "";
  for (const part of parts) {
    current = joinFolderPath(current, part);
    if (!node.folders.has(part)) {
      node.folders.set(part, createGraphTreeNode(part, current));
    }
    node = node.folders.get(part)!;
  }
  return node;
}

function buildGraphTree(
  graphs: Record<string, { name: string; folderPath?: string }>,
  folders: Array<{ name: string; folderPath: string }>
): GraphTreeNode {
  const root = createGraphTreeNode();
  for (const folder of folders) {
    ensureFolder(root, folder.folderPath);
  }
  for (const [id, graph] of Object.entries(graphs)) {
    ensureFolder(root, graph.folderPath ?? "").graphs.push([id, graph]);
  }
  return root;
}

const Sidebar = forwardRef<HTMLDivElement>((_, ref) => {
  const { t } = useTranslation();
  useContext(GroupContext);
  const sidebarNode = useLayoutStore((s) => s.nodes["sidebar"]);
  const currentTab = sidebarNode?.data?.currentTab as "graphs" | "variables" | "data" | "commands" | null;

  const {
    variables: graphVariables,
    Variables: allVariables,
    selectedItemId,
    selectedItemType,
    setSelectedInfo,
    addVariable,
    updateVariable,
    deleteVariable,
    promoteVariable,
    demoteVariable,
    functions,
    addFunction,
    deleteFunction,
    events,
    addEvent,
    deleteEvent,
    dataframes,
    triggerImportData,
    openGraph,
  } = useEditorGroup();

  const toggleSection = useSidebarStore((s) => s.toggleSection);
  const expandedSections = useSidebarStore((s) => s.expandedSections);
  const isSectionExpanded = useCallback(
    (key: string, defaultExpanded = true) => expandedSections[key] ?? defaultExpanded,
    [expandedSections]
  );

  const listRef = useRef<HTMLDivElement>(null);
  const graphFolders = useGraphMetaStore((s) => s.graphFolders);
  const {
    contextMenu,
    closeContextMenu,
    openContextMenu,
    inputDialog,
    setInputDialog,
    openInputDialog,
    submitInputDialog,
  } = useSidebarContextMenu();

  const activeEditorNode = useLayoutStore((s) =>
    s.activeEditorGroupId ? s.nodes[s.activeEditorGroupId] : null
  );
  const activeTabId = activeEditorNode?.data?.activeTabId || null;

  const refreshProjectIndex = useCallback(async () => {
    await useProjectIOStore.getState().syncFromBackend();
  }, []);

  // Graphs > Variable: 只显示当前选择的 graph 的 variable 和 global variable
  const { Variables: globalVariables, graphScopeVariables } = (() => {
    const global: Record<string, { name: string; dataType?: unknown }> = {};
    const local: Record<string, { name: string; dataType?: unknown }> = {};
    for (const [id, v] of Object.entries(allVariables)) {
      const scope = (v as { scope?: { type: string; eventId?: string; functionId?: string } }).scope;
      if (scope?.type === "global") {
        global[id] = v as { name: string; dataType?: unknown };
      } else if (
        activeTabId &&
        scope &&
        (scope.eventId === activeTabId || scope.functionId === activeTabId)
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
      const scope = (v as { scope?: { type: string; eventId?: string; functionId?: string } }).scope;
      const data = v as { name: string; dataType?: unknown };
      if (scope?.type === "global") {
        global[id] = data;
      } else {
        const graphId = scope?.eventId ?? scope?.functionId;
        if (graphId) {
          if (!byGraph[graphId]) {
            const meta = events[graphId] ?? functions[graphId];
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
  const graphVarsCount = Object.keys(graphScopeVariables).length + Object.keys(globalVariables).length;
  const dataframesCount = Object.keys(dataframes || {}).length;

  const prevCounts = useRef({
    events: eventsCount,
    functions: functionsCount,
    variables: graphVarsCount,
    dataframes: dataframesCount,
  });

  useEffect(() => {
    const isAdded =
      eventsCount > prevCounts.current.events ||
      functionsCount > prevCounts.current.functions ||
      graphVarsCount > prevCounts.current.variables ||
      dataframesCount > prevCounts.current.dataframes;

    if (isAdded && listRef.current) {
      listRef.current.scrollTo({ top: listRef.current.scrollHeight, behavior: "smooth" });
    }
    prevCounts.current = {
      events: eventsCount,
      functions: functionsCount,
      variables: graphVarsCount,
      dataframes: dataframesCount,
    };
  }, [eventsCount, functionsCount, graphVarsCount, dataframesCount]);

  const createGraphInFolder = useCallback(async (type: GraphResourceType, folderPath = "") => {
    if (type === "event") {
      await GraphService.createEvent("New Event", folderPath);
    } else {
      await GraphService.createFunction("New Function", folderPath);
    }
    await refreshProjectIndex();
  }, [refreshProjectIndex]);

  const createFolderInFolder = useCallback((type: GraphResourceType, parentFolderPath = "") => {
    openInputDialog("New Folder", "New Folder", async (name) => {
      await GraphService.createGraphFolder(type, joinFolderPath(parentFolderPath, name));
      await refreshProjectIndex();
    }, "Create");
  }, [openInputDialog, refreshProjectIndex]);

  const renameGraphItem = useCallback((id: string, name: string, type: GraphResourceType) => {
    openInputDialog("Rename", name, async (nextName) => {
      if (type === "event") {
        await GraphService.updateEvent(id, { name: nextName } as any);
      } else {
        await GraphService.updateFunction(id, { name: nextName } as any);
      }
      await refreshProjectIndex();
    }, "Rename");
  }, [openInputDialog, refreshProjectIndex]);

  const deleteGraphItem = useCallback(async (id: string, type: GraphResourceType) => {
    if (type === "event") {
      await deleteEvent(id);
    } else {
      await deleteFunction(id);
    }
    await refreshProjectIndex();
  }, [deleteEvent, deleteFunction, refreshProjectIndex]);

  const duplicateGraphItem = useCallback(async (id: string) => {
    await GraphService.duplicateGraph(id);
    await refreshProjectIndex();
  }, [refreshProjectIndex]);

  const renameFolderItem = useCallback((type: GraphResourceType, folderPath: string, name: string) => {
    openInputDialog("Rename Folder", name, async (nextName) => {
      await GraphService.renameGraphFolder(type, folderPath, nextName);
      await refreshProjectIndex();
    }, "Rename");
  }, [openInputDialog, refreshProjectIndex]);

  const deleteFolderItem = useCallback(async (type: GraphResourceType, folderPath: string) => {
    await GraphService.deleteGraphFolder(type, folderPath);
    await refreshProjectIndex();
  }, [refreshProjectIndex]);

  const renameVariableItem = useCallback((id: string, name: string) => {
    openInputDialog("Rename Variable", name, async (nextName) => {
      await updateVariable(id, { name: nextName } as any);
    }, "Rename");
  }, [openInputDialog, updateVariable]);

  const contextMenuSections = buildSidebarContextMenuSections(contextMenu, {
    openGraph,
    createGraphInFolder,
    createFolderInFolder,
    renameGraphItem,
    deleteGraphItem,
    duplicateGraphItem,
    renameFolderItem,
    deleteFolderItem,
    addVariable,
    renameVariableItem,
    deleteVariable,
  });

  const renderItem = (
    id: string,
    name: string,
    type: "variable" | "function" | "event" | "data",
    extra?: { dataType?: unknown; isGlobal?: boolean; folderPath?: string },
    readOnly?: boolean,
    nested?: boolean | number,
    onContextMenu?: (e: React.MouseEvent) => void
  ) => {
    const isSelected = selectedItemId === id && selectedItemType === type;
    const dragData = readOnly ? null : buildSidebarDragData(id, name, type, extra as { dataType?: DataType | string; folderPath?: string } | undefined);
    const indentDepth = typeof nested === "number" ? nested : nested ? 1 : 0;

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
        onContextMenu={onContextMenu}
        className={`
          group flex items-center gap-2 pr-2 py-1.5 transition-colors duration-150 ease-out
          ${isSelected
            ? "bg-[var(--sidebar-item-active)] text-gray-200"
            : "hover:bg-[var(--sidebar-hover)] text-gray-400"}
        `}
        style={{ paddingLeft: 16 + indentDepth * 16 }}
      >
        <span
          className="shrink-0 flex items-center justify-center"
          style={{
            color: type === "event"
                ? TYPE_ICON_COLORS.event
                : type === "function"
                  ? TYPE_ICON_COLORS.function
                  : type === "variable"
                      ? extra?.isGlobal
                        ? TYPE_ICON_COLORS.variableGlobal
                        : TYPE_ICON_COLORS.variable
                      : type === "data"
                        ? TYPE_ICON_COLORS.data
                        : "rgba(156,163,175,0.8)",
          }}
        >
          {type === "event" && <VscSymbolEvent size={12} />}
          {type === "function" && <VscSymbolMethod size={12} />}
          {type === "variable" && <VscSymbolVariable size={12} />}
          {type === "data" && <VscDatabase size={12} />}
        </span>
        <span className="flex-1 text-[12px] font-normal tracking-tight truncate">{name}</span>
        {(type === "event" || type === "function") && (
          <Button
            type="button"
            variant="ghost"
            size="icon-xs"
            onClick={(e) => {
              e.stopPropagation();
              openGraph(id, name, type);
            }}
            className={`opacity-0 transition-opacity group-hover:opacity-100 ${isSelected ? "text-gray-200" : "text-gray-500"}`}
            title={t("sidebar.open")}
          >
            <VscChevronRight size={11} />
          </Button>
        )}
        {type === "variable" && !readOnly && (
          <>
            {!extra?.isGlobal ? (
              <Button
                type="button"
                variant="ghost"
                size="icon-xs"
                onClick={(e) => {
                  e.stopPropagation();
                  promoteVariable(id);
                }}
                className={`opacity-0 transition-opacity group-hover:opacity-100 ${isSelected ? "text-gray-200" : "text-gray-500"}`}
                title={t("sidebar.promoteToGlobal")}
              >
                <VscEye size={11} />
              </Button>
            ) : (
              <Button
                type="button"
                variant="ghost"
                size="icon-xs"
                onClick={(e) => {
                  e.stopPropagation();
                  demoteVariable(id);
                }}
                className={`opacity-0 transition-opacity group-hover:opacity-100 ${isSelected ? "text-gray-200" : "text-gray-500"}`}
                title={t("sidebar.demoteToLocal")}
              >
                <VscEyeClosed size={11} />
              </Button>
            )}
            <span
              className={`text-[10px] font-normal px-1 py-0.5 flex items-center gap-1 ${isSelected ? "bg-white/[0.12]" : "bg-white/[0.04]"}`}
              style={{ color: safeDataTypeColor(extra?.dataType) }}
            >
              {safeDataTypeDisplay(extra?.dataType)}
              {extra?.dataType &&
                typeof extra.dataType === "object" &&
                "kind" in extra.dataType &&
                (extra.dataType as DataType).kind === "Array"
                  ? <span className="text-[8px]">[]</span>
                  : null}
            </span>
          </>
        )}
        {type === "variable" && readOnly && (
          <span
            className={`text-[10px] font-normal px-1 py-0.5 flex items-center gap-1 ${isSelected ? "bg-white/[0.12]" : "bg-white/[0.04]"}`}
            style={{ color: safeDataTypeColor(extra?.dataType) }}
          >
            {safeDataTypeDisplay(extra?.dataType)}
          </span>
        )}
      </SidebarDraggableItem>
    );
  };

  const renderGraphTree = (
    type: GraphResourceType,
    graphs: Record<string, { name: string; folderPath?: string }>,
    depth = 0,
    node = buildGraphTree(
      graphs,
      graphFolders.filter((folder) => folder.type === type)
    )
  ): React.ReactNode => {
    const folderEntries = Array.from(node.folders.values()).sort((a, b) => a.name.localeCompare(b.name));
    const graphEntries = [...node.graphs].sort((a, b) => a[1].name.localeCompare(b[1].name));
    return (
      <>
        {folderEntries.map((folder) => (
          <CollapsibleSection
            key={`${type}-folder-${folder.path}`}
            label={folder.name}
            expanded={isSectionExpanded(`graphs_${type}_folder_${folder.path}`)}
            onToggle={() => toggleSection(`graphs_${type}_folder_${folder.path}`)}
            onAdd={() => void createGraphInFolder(type, folder.path)}
            indentDepth={depth}
            dropTarget={{ graphType: type, folderPath: folder.path }}
            onContextMenu={(e) => openContextMenu(e, {
              type: "folder",
              graphType: type,
              folderPath: folder.path,
              name: folder.name,
            })}
            headerContent={
              <div className="flex items-center gap-2 min-w-0 text-gray-500">
                <VscFolder size={12} className="shrink-0" />
                <span className="flex-1 text-[12px] tracking-tight truncate">{folder.name}</span>
              </div>
            }
          >
            {renderGraphTree(type, {}, depth + 1, folder)}
          </CollapsibleSection>
        ))}
        {graphEntries.map(([id, data]) =>
          renderItem(
            id,
            data.name,
            type,
            { folderPath: data.folderPath },
            false,
            depth,
            (e) => openContextMenu(e, {
              type: "graph",
              id,
              name: data.name,
              graphType: type,
              folderPath: data.folderPath,
            })
          )
        )}
      </>
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
        <div className="px-3 border-b border-border bg-[var(--workbench-bg)]/50 flex justify-between items-center shrink-0" style={{ height: 'var(--titlebar-height)' }}>
          <span className="text-[10px] font-black text-gray-500 uppercase tracking-widest">
            {currentTab === "graphs" ? "Graphs" : currentTab === "variables" ? "Variables" : currentTab === "data" ? "Data" : currentTab === "commands" ? "Commands" : ""}
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
                dropTarget={{ graphType: "event", folderPath: "" }}
                onHeaderContextMenu={(e) => openContextMenu(e, { type: "section", graphType: "event" })}
                onContentContextMenu={(e) => openContextMenu(e, { type: "section", graphType: "event" })}
              >
                <div
                  className="flex min-h-full flex-col"
                  onContextMenu={(e) => openContextMenu(e, { type: "section", graphType: "event" })}
                >
                  {renderGraphTree("event", events as Record<string, { name: string; folderPath?: string }>)}
                  {Object.keys(events).length === 0 && !graphFolders.some((folder) => folder.type === "event") && (
                    <div className="text-[12px] text-gray-500/70 pl-4 py-1.5">No events</div>
                  )}
                </div>
              </StackedCollapsibleSection>

              <StackedCollapsibleSection
                label="Function"
                expanded={isSectionExpanded("graphsFunction")}
                onToggle={() => toggleSection("graphsFunction")}
                onAdd={addFunction}
                dropTarget={{ graphType: "function", folderPath: "" }}
                onHeaderContextMenu={(e) => openContextMenu(e, { type: "section", graphType: "function" })}
                onContentContextMenu={(e) => openContextMenu(e, { type: "section", graphType: "function" })}
              >
                <div
                  className="flex min-h-full flex-col"
                  onContextMenu={(e) => openContextMenu(e, { type: "section", graphType: "function" })}
                >
                  {renderGraphTree("function", functions as Record<string, { name: string; folderPath?: string }>)}
                  {Object.keys(functions).length === 0 && !graphFolders.some((folder) => folder.type === "function") && (
                    <div className="text-[12px] text-gray-500/70 pl-4 py-1.5">No functions</div>
                  )}
                </div>
              </StackedCollapsibleSection>

              <StackedCollapsibleSection
                label="Variable"
                expanded={isSectionExpanded("graphsVariable")}
                onToggle={() => toggleSection("graphsVariable")}
                onAdd={() => addVariable("New Variable", "Int32", false)}
              >
                  {Object.keys(globalVariables).length > 0 &&
                    Object.entries(globalVariables).map(([id, data]: [string, { name: string }]) =>
                      renderItem(id, data.name, "variable", { ...data, isGlobal: true }, false, false, (e) =>
                        openContextMenu(e, { type: "variable", id, name: data.name })
                      )
                    )}
                  {Object.entries(graphScopeVariables).map(([id, data]: [string, { name: string }]) => {
                    if (id in globalVariables) return null;
                    return renderItem(id, data.name, "variable", { ...data, isGlobal: false }, false, false, (e) =>
                      openContextMenu(e, { type: "variable", id, name: data.name })
                    );
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
                        headerContentToggles={false}
                        headerActive={isSelected}
                        headerContent={
                          <div className="flex items-center gap-2 min-w-0">
                            <SidebarDraggableItem
                              id={id}
                              dragData={dragData}
                              onClick={(e) => {
                                e.stopPropagation();
                                setSelectedInfo(id, "data");
                              }}
                              onDoubleClick={(e) => {
                                e.stopPropagation();
                                openDataViewWindow(id);
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
                            <Button
                              type="button"
                              variant="ghost"
                              size="icon-xs"
                              onClick={(e) => {
                                e.stopPropagation();
                                openDataViewWindow(id);
                              }}
                              className="shrink-0 text-gray-500 opacity-0 transition-opacity group-hover:opacity-100"
                              title={t("sidebar.viewInDataViewer")}
                            >
                              <VscEye size={12} />
                            </Button>
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
                    <div className="text-[12px] text-gray-500/70 pl-4 py-1.5">{t("sidebar.noData")}</div>
                  )}
              </StackedCollapsibleSection>
            </div>
          )}
          {currentTab === "commands" && (
            <CommandsPanel activeTabId={activeTabId} />
          )}
        </div>
      </div>
      {contextMenu && (
        <ContextMenu
          position={{ x: contextMenu.x, y: contextMenu.y }}
          sections={contextMenuSections}
          onClose={closeContextMenu}
        />
      )}
      <Dialog open={!!inputDialog} onOpenChange={(open) => !open && setInputDialog(null)}>
        {inputDialog && (
          <DialogContent className="max-w-[320px]">
            <DialogHeader className="border-b border-border bg-muted/20">
              <DialogTitle>{inputDialog.title}</DialogTitle>
            </DialogHeader>
            <form
              onSubmit={(e) => {
                e.preventDefault();
                void submitInputDialog();
              }}
            >
              <div className="px-5 py-4">
            <Input
              autoFocus
              value={inputDialog.value}
              onChange={(e) => setInputDialog({ ...inputDialog, value: e.target.value })}
              onKeyDown={(e) => {
                if (e.key === "Escape") setInputDialog(null);
              }}
              className="h-8 text-xs"
            />
              </div>
            <DialogFooter>
              <Button type="button" variant="ghost" size="sm" onClick={() => setInputDialog(null)}>
                Cancel
              </Button>
              <Button type="submit" size="sm">
                {inputDialog.submitLabel ?? "OK"}
              </Button>
            </DialogFooter>
            </form>
          </DialogContent>
        )}
      </Dialog>
    </div>
  );
});

const COMMAND_LABELS: Record<string, string> = {
  MoveNodes: "Move Nodes",
  SetPinValue: "Set Pin Value",
  ConnectPins: "Connect Pins",
  DisconnectPin: "Disconnect Pin",
  CreateNode: "Create Node",
  DeleteNodes: "Delete Nodes",
  Composite: "Composite",
};

function formatTime(ts: number): string {
  const d = new Date(ts);
  return `${d.getHours().toString().padStart(2, "0")}:${d.getMinutes().toString().padStart(2, "0")}:${d.getSeconds().toString().padStart(2, "0")}`;
}

const EMPTY_STACK: HistoryEntry[] = [];

function CommandsPanel({ activeTabId }: { activeTabId: string | null }) {
  const { t } = useTranslation();
  const undoStack = useHistoryStore((s) =>
    activeTabId ? s.histories[activeTabId]?.undoStack ?? EMPTY_STACK : EMPTY_STACK
  );
  const redoStack = useHistoryStore((s) =>
    activeTabId ? s.histories[activeTabId]?.redoStack ?? EMPTY_STACK : EMPTY_STACK
  );

  if (!activeTabId) {
    return (
      <div className="flex flex-col flex-1 min-h-0">
        <div className="text-[12px] text-gray-500/60 pl-4 py-3">{t("sidebar.noActiveGraph")}</div>
      </div>
    );
  }

  const reversedUndo = [...undoStack].reverse();

  return (
    <div className="flex flex-col flex-1 min-h-0">
      <StackedCollapsibleSection
        label={`${t("common.undo")} (${undoStack.length})`}
        expanded={true}
        onToggle={() => {}}
      >
        {reversedUndo.length > 0 ? reversedUndo.map((entry, i) => (
          <div
            key={entry.id}
            className={`flex items-center gap-2 px-4 py-1.5 text-gray-400 ${i === 0 ? "bg-white/[0.04]" : ""}`}
          >
            <VscDiscard size={11} className="shrink-0 text-gray-500" />
            <span className="flex-1 text-[12px] tracking-tight truncate">
              {t(`sidebar.commands.${entry.commandType}`, { defaultValue: COMMAND_LABELS[entry.commandType] ?? entry.commandType })}
            </span>
            <span className="text-[10px] text-gray-600 shrink-0">{formatTime(entry.timestamp)}</span>
          </div>
        )) : (
          <div className="text-[12px] text-gray-500/60 pl-4 py-1.5">—</div>
        )}
      </StackedCollapsibleSection>

      <StackedCollapsibleSection
        label={`${t("common.redo")} (${redoStack.length})`}
        expanded={true}
        onToggle={() => {}}
      >
        {redoStack.length > 0 ? redoStack.map((entry) => (
          <div
            key={entry.id}
            className="flex items-center gap-2 px-4 py-1.5 text-gray-500"
          >
            <VscRedo size={11} className="shrink-0 text-gray-600" />
            <span className="flex-1 text-[12px] tracking-tight truncate">
              {t(`sidebar.commands.${entry.commandType}`, { defaultValue: COMMAND_LABELS[entry.commandType] ?? entry.commandType })}
            </span>
            <span className="text-[10px] text-gray-600 shrink-0">{formatTime(entry.timestamp)}</span>
          </div>
        )) : (
          <div className="text-[12px] text-gray-500/60 pl-4 py-1.5">—</div>
        )}
      </StackedCollapsibleSection>
    </div>
  );
}

export default Sidebar;
