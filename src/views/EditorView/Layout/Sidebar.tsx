import { forwardRef, useCallback, useContext, useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import { useEditorGroup, GroupContext } from "@/features/application/editor";
import {
  VscEye,
  VscEyeClosed,
  VscChevronRight,
  VscDatabase,
  VscSymbolEvent,
  VscSymbolMethod,
  VscSymbolVariable,
  VscDiscard,
  VscRedo,
  VscFolder,
  VscGraphLine,
} from "react-icons/vsc";
import { useLayoutStore } from "@/features/core/layout/layoutStore";
import { useHistoryStore } from "@/features/core/history";
import type { HistoryEntry } from "@/features/core/history";
import { useSidebarStore } from "@/features/core/sidebar";
import { buildSidebarDragData } from "@/features/application/sidebar";
import { ensureDetailVisible } from "@/features/application/editor/ensureDetailVisible";
import { TYPE_ICON_COLORS } from "@/features/domain/sidebar";
import type { DataType } from "@/shared/types/domain/dataType";
import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { Input } from "@/components/ui/input";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { cn } from "@/lib/utils";
import {
  DEFAULT_FOLDER_NAME,
  DEFAULT_VARIABLE_NAME,
} from "@/shared/constants/defaultResourceNames";
import { formatErrorMessage } from "@/shared/utils/formatErrorMessage";
import { ContextMenu } from "@/shared/ui/contextMenu";
import { ProjectService } from "@/services/project/projectService";
import { useResourceStore } from "@/features/core/resource";
import { uiStore } from "@/features/core/ui/UIStore";
import { useWorksheetStore } from "@/features/core/worksheet/worksheetStore";
import {
  createGraphFolderResource,
  createGraphResource,
  deleteGraphFolderResource,
  deleteResource,
  duplicateGraphResource,
  renameGraphFolderResource,
  renameResource,
} from "@/features/application/resource/resourceActions";
import { openDataViewWindow, safeDataTypeColor, safeDataTypeDisplay } from "./sidebarUtils";
import {
  buildSidebarContextMenuSections,
  useSidebarContextMenu,
  type GraphResourceType,
} from "./sidebarContextMenu";
import {
  SidebarCollapsibleSection,
  SidebarListItem,
  sidebarItemIndent,
  sidebarItemRowClass,
  sidebarRowActionClass,
} from "./sidebarUi";

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
  const currentTab = sidebarNode?.data?.currentTab as "graphs" | "variables" | "data" | "commands" | "charts" | null;

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
    events,
    dataframes,
    triggerImportData,
    openGraph,
    addWorksheet,
    openWorksheet,
  } = useEditorGroup();

  const worksheets = useWorksheetStore((s) => s.index);

  const toggleSection = useSidebarStore((s) => s.toggleSection);
  const expandedSections = useSidebarStore((s) => s.expandedSections);
  const isSectionExpanded = useCallback(
    (key: string, defaultExpanded = true) => expandedSections[key] ?? defaultExpanded,
    [expandedSections]
  );

  const listRef = useRef<HTMLDivElement>(null);
  const graphFolders = useResourceStore((s) => s.graphFolders);
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
    await createGraphResource(type, folderPath);
  }, []);

  const createRootEvent = useCallback(() => {
    void createGraphResource("event");
  }, []);

  const createRootFunction = useCallback(() => {
    void createGraphResource("function");
  }, []);

  const createFolderInFolder = useCallback((type: GraphResourceType, parentFolderPath = "") => {
    const title = t("contextMenu.dialog.newFolderTitle");
    openInputDialog(title, DEFAULT_FOLDER_NAME, async (name) => {
      await createGraphFolderResource(type, joinFolderPath(parentFolderPath, name));
    }, t("contextMenu.dialog.createSubmit"));
  }, [openInputDialog, t]);

  const renameGraphItem = useCallback((id: string, name: string, type: GraphResourceType) => {
    openInputDialog(t("contextMenu.dialog.renameGraphTitle"), name, async (nextName) => {
      await renameResource({ id, kind: type }, nextName);
    }, t("contextMenu.dialog.renameSubmit"));
  }, [openInputDialog, t]);

  const deleteGraphItem = useCallback(async (id: string, type: GraphResourceType) => {
    await deleteResource({ id, kind: type });
  }, []);

  const duplicateGraphItem = useCallback(async (id: string) => {
    await duplicateGraphResource(id);
  }, []);

  const renameFolderItem = useCallback((type: GraphResourceType, folderPath: string, name: string) => {
    openInputDialog(t("contextMenu.dialog.renameFolderTitle"), name, async (nextName) => {
      await renameGraphFolderResource(type, folderPath, nextName);
    }, t("contextMenu.dialog.renameSubmit"));
  }, [openInputDialog, t]);

  const deleteFolderItem = useCallback(async (type: GraphResourceType, folderPath: string) => {
    await deleteGraphFolderResource(type, folderPath);
  }, []);

  const renameVariableItem = useCallback((id: string, name: string) => {
    openInputDialog(t("contextMenu.dialog.renameVariableTitle"), name, async (nextName) => {
      await renameResource({ id, kind: "variable" }, nextName);
    }, t("contextMenu.dialog.renameSubmit"));
  }, [openInputDialog, t]);

  const renameDatabaseItem = useCallback((id: string, name: string) => {
    openInputDialog(t("contextMenu.dialog.renameDataTitle"), name, async (nextName) => {
      await renameResource({ id, kind: "database" }, nextName);
    }, t("contextMenu.dialog.renameSubmit"));
  }, [openInputDialog, t]);

  const deleteVariableItem = useCallback(async (id: string) => {
    await deleteResource({ id, kind: "variable" });
  }, []);

  const deleteDatabaseItem = useCallback(async (id: string) => {
    await deleteResource({ id, kind: "database" });
  }, []);

  const revealInExplorer = useCallback(async (request: Parameters<typeof ProjectService.revealProjectResource>[0]) => {
    try {
      await ProjectService.revealProjectResource(request);
    } catch (error) {
      uiStore.showToast(
        t("contextMenu.sidebar.revealInExplorerFailed", {
          error: formatErrorMessage(error, "Unknown error"),
        }),
        "error",
      );
    }
  }, [t]);

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
    deleteVariable: deleteVariableItem,
    openDatabase: openDataViewWindow,
    renameDatabaseItem,
    deleteDatabaseItem,
    importData: triggerImportData,
    openWorksheet,
    revealInExplorer,
  }, t);

  const openVariableContextMenu = useCallback(
    (e: React.MouseEvent, id: string, name: string) => {
      openContextMenu(e, { type: "variable", id, name });
    },
    [openContextMenu]
  );

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

    const iconColor =
      type === "event"
        ? TYPE_ICON_COLORS.event
        : type === "function"
          ? TYPE_ICON_COLORS.function
          : type === "variable"
            ? extra?.isGlobal
              ? TYPE_ICON_COLORS.variableGlobal
              : TYPE_ICON_COLORS.variable
            : TYPE_ICON_COLORS.data;

    const icon =
      type === "event" ? <VscSymbolEvent size={12} style={{ color: iconColor }} />
      : type === "function" ? <VscSymbolMethod size={12} style={{ color: iconColor }} />
      : type === "variable" ? <VscSymbolVariable size={12} style={{ color: iconColor }} />
      : <VscDatabase size={12} style={{ color: iconColor }} />;

    return (
      <SidebarListItem
        key={id}
        id={id}
        dragData={dragData}
        isSelected={isSelected}
        indentDepth={indentDepth}
        icon={icon}
        label={name}
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
        trailing={
          <>
            {(type === "event" || type === "function") && (
              <Tooltip>
                <TooltipTrigger asChild>
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon-xs"
                    onClick={(e) => {
                      e.stopPropagation();
                      openGraph(id, name, type);
                    }}
                    className={sidebarRowActionClass(isSelected)}
                  >
                    <VscChevronRight size={11} />
                  </Button>
                </TooltipTrigger>
                <TooltipContent side="top">{t("sidebar.open")}</TooltipContent>
              </Tooltip>
            )}
            {type === "variable" && !readOnly && (
              <>
                {!extra?.isGlobal ? (
                  <Tooltip>
                    <TooltipTrigger asChild>
                      <Button
                        type="button"
                        variant="ghost"
                        size="icon-xs"
                        onClick={(e) => {
                          e.stopPropagation();
                          promoteVariable(id);
                        }}
                        className={sidebarRowActionClass(isSelected)}
                      >
                        <VscEye size={11} />
                      </Button>
                    </TooltipTrigger>
                    <TooltipContent side="top">{t("sidebar.promoteToGlobal")}</TooltipContent>
                  </Tooltip>
                ) : (
                  <Tooltip>
                    <TooltipTrigger asChild>
                      <Button
                        type="button"
                        variant="ghost"
                        size="icon-xs"
                        onClick={(e) => {
                          e.stopPropagation();
                          demoteVariable(id);
                        }}
                        className={sidebarRowActionClass(isSelected)}
                      >
                        <VscEyeClosed size={11} />
                      </Button>
                    </TooltipTrigger>
                    <TooltipContent side="top">{t("sidebar.demoteToLocal")}</TooltipContent>
                  </Tooltip>
                )}
                <span
                  className={cn(
                    "flex items-center gap-1 px-1 py-0.5 text-[10px] font-normal",
                    isSelected ? "bg-white/[0.12]" : "bg-sidebar-accent/50",
                  )}
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
                className={cn(
                  "flex items-center gap-1 px-1 py-0.5 text-[10px] font-normal",
                  isSelected ? "bg-white/[0.12]" : "bg-sidebar-accent/50",
                )}
                style={{ color: safeDataTypeColor(extra?.dataType) }}
              >
                {safeDataTypeDisplay(extra?.dataType)}
              </span>
            )}
          </>
        }
      />
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
          <SidebarCollapsibleSection
            key={`${type}-folder-${folder.path}`}
            variant="nested"
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
            leading={<VscFolder size={12} className="text-muted-foreground" />}
          >
            {renderGraphTree(type, {}, depth + 1, folder)}
          </SidebarCollapsibleSection>
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

  const renderDataItem = (id: string, name: string, data: unknown) => {
    const isLoading = (data as { loading?: unknown }).loading === true;
    const loadError = (data as { loadError?: unknown }).loadError;
    const isSelected = selectedItemId === id && selectedItemType === "data";

    return (
      <SidebarListItem
        key={id}
        id={id}
        dragData={buildSidebarDragData(id, name, "data")}
        isSelected={isSelected}
        icon={<VscDatabase size={12} style={{ color: TYPE_ICON_COLORS.data }} />}
        label={name}
        onClick={(e) => {
          e.stopPropagation();
          setSelectedInfo(id, "data");
        }}
        onDoubleClick={(e) => {
          e.stopPropagation();
          openDataViewWindow(id);
        }}
        onContextMenu={(e) => {
          e.preventDefault();
          e.stopPropagation();
          openContextMenu(e, { type: "database", id, name });
        }}
        trailing={
          <>
            {isLoading && (
              <Tooltip>
                <TooltipTrigger asChild>
                  <span className="inline-block h-1.5 w-1.5 shrink-0 rounded-full bg-amber-400 animate-pulse" />
                </TooltipTrigger>
                <TooltipContent side="top">{t("sidebar.dataLoading")}</TooltipContent>
              </Tooltip>
            )}
            {!isLoading && typeof loadError === "string" && loadError.length > 0 && (
              <Tooltip>
                <TooltipTrigger asChild>
                  <span className="inline-block h-1.5 w-1.5 shrink-0 rounded-full bg-red-500" />
                </TooltipTrigger>
                <TooltipContent side="top">{String(loadError)}</TooltipContent>
              </Tooltip>
            )}
            <Tooltip>
              <TooltipTrigger asChild>
                <Button
                  type="button"
                  variant="ghost"
                  size="icon-xs"
                  onClick={(e) => {
                    e.stopPropagation();
                    openDataViewWindow(id);
                  }}
                  className={sidebarRowActionClass(isSelected)}
                >
                  <VscEye size={11} />
                </Button>
              </TooltipTrigger>
              <TooltipContent side="top">{t("sidebar.viewInDataViewer")}</TooltipContent>
            </Tooltip>
          </>
        }
      />
    );
  };

  const renderWorksheetItem = (id: string, name: string) => {
    const isSelected = selectedItemId === id && selectedItemType === "worksheet";
    return (
      <SidebarListItem
        key={id}
        id={id}
        isSelected={isSelected}
        icon={<VscGraphLine size={12} style={{ color: TYPE_ICON_COLORS.worksheet }} />}
        label={name}
        onClick={(e) => {
          e.stopPropagation();
          setSelectedInfo(id, "worksheet");
          ensureDetailVisible();
        }}
        onDoubleClick={(e) => {
          e.stopPropagation();
          void openWorksheet(id, name);
        }}
        onContextMenu={(e) => {
          e.preventDefault();
          e.stopPropagation();
          openContextMenu(e, { type: "worksheet", id, name });
        }}
        trailing={
          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                type="button"
                variant="ghost"
                size="icon-xs"
                onClick={(e) => {
                  e.stopPropagation();
                  void openWorksheet(id, name);
                }}
                className={sidebarRowActionClass(isSelected)}
              >
                <VscChevronRight size={11} />
              </Button>
            </TooltipTrigger>
            <TooltipContent side="top">{t("sidebar.open")}</TooltipContent>
          </Tooltip>
        }
      />
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
          <span className="text-[10px] font-black text-muted-foreground uppercase tracking-widest">
            {currentTab === "graphs"
              ? "Graphs"
              : currentTab === "variables"
                ? "Variables"
                : currentTab === "data"
                  ? "Data"
                  : currentTab === "commands"
                    ? "Commands"
                    : currentTab === "charts"
                      ? t("activityBar.charts")
                      : ""}
          </span>
        </div>

        <div className="flex flex-col flex-1 min-h-0 overflow-hidden p-0">
          {currentTab === "graphs" && (
            <div ref={listRef} className="flex flex-col flex-1 min-h-0">
              <SidebarCollapsibleSection variant="stacked"
                label="Event"
                expanded={isSectionExpanded("graphsEvent")}
                onToggle={() => toggleSection("graphsEvent")}
                onAdd={createRootEvent}
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
                    <div className="text-[12px] text-muted-foreground/70 pl-4 py-1.5">No events</div>
                  )}
                </div>
              </SidebarCollapsibleSection>

              <SidebarCollapsibleSection variant="stacked"
                label="Function"
                expanded={isSectionExpanded("graphsFunction")}
                onToggle={() => toggleSection("graphsFunction")}
                onAdd={createRootFunction}
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
                    <div className="text-[12px] text-muted-foreground/70 pl-4 py-1.5">No functions</div>
                  )}
                </div>
              </SidebarCollapsibleSection>

              <SidebarCollapsibleSection variant="stacked"
                label="Variable"
                expanded={isSectionExpanded("graphsVariable")}
                onToggle={() => toggleSection("graphsVariable")}
                onAdd={() => addVariable(DEFAULT_VARIABLE_NAME, "Int32", false)}
                onHeaderContextMenu={(e) => openContextMenu(e, { type: "variableSection", isGlobal: false })}
                onContentContextMenu={(e) => openContextMenu(e, { type: "variableSection", isGlobal: false })}
              >
                  {Object.keys(globalVariables).length > 0 &&
                    Object.entries(globalVariables).map(([id, data]: [string, { name: string }]) =>
                      renderItem(id, data.name, "variable", { ...data, isGlobal: true }, false, false, (e) =>
                        openVariableContextMenu(e, id, data.name)
                      )
                    )}
                  {Object.entries(graphScopeVariables).map(([id, data]: [string, { name: string }]) => {
                    if (id in globalVariables) return null;
                    return renderItem(id, data.name, "variable", { ...data, isGlobal: false }, false, false, (e) =>
                      openVariableContextMenu(e, id, data.name)
                    );
                  })}
                  {Object.keys(graphScopeVariables).length === 0 && Object.keys(globalVariables).length === 0 && (
                    <div className="text-[12px] text-muted-foreground/70 pl-4 py-1.5">No variables</div>
                  )}
              </SidebarCollapsibleSection>
            </div>
          )}

          {currentTab === "variables" && (
            <div className="flex flex-col flex-1 min-h-0">
              <SidebarCollapsibleSection variant="stacked"
                label="Global"
                expanded={isSectionExpanded("variablesGlobal")}
                onToggle={() => toggleSection("variablesGlobal")}
                onAdd={() => addVariable(DEFAULT_VARIABLE_NAME, "Int32", true)}
                onHeaderContextMenu={(e) => openContextMenu(e, { type: "variableSection", isGlobal: true })}
                onContentContextMenu={(e) => openContextMenu(e, { type: "variableSection", isGlobal: true })}
              >
                  {Object.entries(variablesGlobal).map(([id, data]: [string, { name: string }]) =>
                    renderItem(id, data.name, "variable", { ...data, isGlobal: true }, true, false, (e) =>
                      openVariableContextMenu(e, id, data.name)
                    )
                  )}
                {Object.keys(variablesGlobal).length === 0 && (
                  <div className="text-[12px] text-muted-foreground/60 pl-4 py-1.5">—</div>
                )}
              </SidebarCollapsibleSection>

              <SidebarCollapsibleSection variant="stacked"
                label="Local"
                expanded={isSectionExpanded("variablesLocal")}
                onToggle={() => toggleSection("variablesLocal")}
              >
                  {localVariablesByGraph.map(({ graphId, graphName, variables }) => (
                    <SidebarCollapsibleSection
                      key={graphId}
                      variant="nested"
                      label={graphName}
                      expanded={isSectionExpanded(`variablesLocal_${graphId}`)}
                      onToggle={() => toggleSection(`variablesLocal_${graphId}`)}
                    >
                      {Object.entries(variables).map(([id, data]: [string, { name: string }]) =>
                        renderItem(id, data.name, "variable", { ...data, isGlobal: false }, true, true, (e) =>
                          openVariableContextMenu(e, id, data.name)
                        )
                      )}
                    </SidebarCollapsibleSection>
                  ))}
                {localVariablesByGraph.length === 0 && (
                  <div className="text-[12px] text-muted-foreground/60 pl-4 py-1.5">—</div>
                )}
              </SidebarCollapsibleSection>
            </div>
          )}

          {currentTab === "data" && (
            <div className="flex flex-col flex-1 min-h-0">
              <SidebarCollapsibleSection variant="stacked"
                label="Data"
                expanded={isSectionExpanded("dataData")}
                onToggle={() => toggleSection("dataData")}
                onAdd={triggerImportData}
                onHeaderContextMenu={(e) => openContextMenu(e, { type: "dataSection" })}
                onContentContextMenu={(e) => openContextMenu(e, { type: "dataSection" })}
              >
                  {Object.entries(dataframes || {}).map(([id, data]) => {
                    const name = String((data as { name?: unknown }).name ?? "");
                    return renderDataItem(id, name, data);
                  })}
                  {Object.keys(dataframes || {}).length === 0 && (
                    <div className="text-[12px] text-muted-foreground/70 pl-4 py-1.5">{t("sidebar.noData")}</div>
                  )}
              </SidebarCollapsibleSection>
            </div>
          )}
          {currentTab === "commands" && (
            <CommandsPanel activeTabId={activeTabId} />
          )}
          {currentTab === "charts" && (
            <div ref={listRef} className="flex flex-col flex-1 min-h-0">
              <SidebarCollapsibleSection variant="stacked"
                label={t("chartsSidebar.worksheets")}
                expanded={isSectionExpanded("chartsWorksheets")}
                onToggle={() => toggleSection("chartsWorksheets")}
                onAdd={() => void addWorksheet()}
              >
                {worksheets.map((ws) => renderWorksheetItem(ws.id, ws.name))}
                {worksheets.length === 0 && (
                  <div className="text-[12px] text-muted-foreground/70 pl-4 py-1.5">{t("chartsSidebar.noWorksheets")}</div>
                )}
              </SidebarCollapsibleSection>
            </div>
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
        <div className="text-[12px] text-muted-foreground/60 pl-4 py-3">{t("sidebar.noActiveGraph")}</div>
      </div>
    );
  }

  const reversedUndo = [...undoStack].reverse();

  return (
    <div className="flex flex-col flex-1 min-h-0">
      <SidebarCollapsibleSection variant="stacked"
        label={`${t("common.undo")} (${undoStack.length})`}
        expanded={true}
        onToggle={() => {}}
      >
        {reversedUndo.length > 0 ? reversedUndo.map((entry, i) => (
          <div
            key={entry.id}
            className={cn(sidebarItemRowClass(i === 0), "pr-2")}
            style={sidebarItemIndent(0)}
          >
            <VscDiscard size={11} className="shrink-0 text-muted-foreground" />
            <span className="min-w-0 flex-1 truncate text-[12px] font-normal tracking-tight">
              {t(`sidebar.commands.${entry.commandType}`, { defaultValue: COMMAND_LABELS[entry.commandType] ?? entry.commandType })}
            </span>
            <span className="shrink-0 text-[10px] text-muted-foreground/60">{formatTime(entry.timestamp)}</span>
          </div>
        )) : (
          <div className="text-[12px] text-muted-foreground/60 pl-4 py-1.5">—</div>
        )}
      </SidebarCollapsibleSection>

      <SidebarCollapsibleSection variant="stacked"
        label={`${t("common.redo")} (${redoStack.length})`}
        expanded={true}
        onToggle={() => {}}
      >
        {redoStack.length > 0 ? redoStack.map((entry) => (
          <div
            key={entry.id}
            className={cn(sidebarItemRowClass(false), "pr-2")}
            style={sidebarItemIndent(0)}
          >
            <VscRedo size={11} className="shrink-0 text-muted-foreground/60" />
            <span className="min-w-0 flex-1 truncate text-[12px] font-normal tracking-tight">
              {t(`sidebar.commands.${entry.commandType}`, { defaultValue: COMMAND_LABELS[entry.commandType] ?? entry.commandType })}
            </span>
            <span className="shrink-0 text-[10px] text-muted-foreground/60">{formatTime(entry.timestamp)}</span>
          </div>
        )) : (
          <div className="text-[12px] text-muted-foreground/60 pl-4 py-1.5">—</div>
        )}
      </SidebarCollapsibleSection>
    </div>
  );
}

export default Sidebar;
