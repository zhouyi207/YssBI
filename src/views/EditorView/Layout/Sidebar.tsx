import { forwardRef, useCallback, useContext, useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import { useEditorGroup } from "@/features/application/editor";
import { GroupContext } from "@/features/core/editor";
import { useDetailTarget } from "@/features/core/editor";
import { useEditorStore } from "@/features/core/editor/stores/useEditorStore";
import { setVariablesGraphScopeFromResource } from "@/features/core/editor/detail/variablesGraphScope";
import {
  VscDatabase,
  VscSymbolEvent,
  VscSymbolMethod,
  VscSymbolVariable,
  VscDiscard,
  VscRedo,
  VscGraphLine,
} from "react-icons/vsc";
import { useLayoutStore } from "@/features/core/layout/layoutStore";
import { useHistoryStore } from "@/features/core/history";
import type { HistoryEntry } from "@/features/core/history";
import { useSidebarStore } from "@/features/core/sidebar";
import { buildSidebarDragData } from "@/features/application/sidebar";
import { deleteWorksheetWithConfirm } from "@/features/application/editor/closeEditorTab";
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
  DEFAULT_VARIABLE_NAME,
} from "@/shared/constants/defaultResourceNames";
import { ContextMenu } from "@/shared/ui/contextMenu";
import {
  renameWorksheetResource,
  revealProjectResourceInExplorer,
} from "@/features/application/sidebar/sidebarResourceActions";
import { useWorksheetStore } from "@/features/core/worksheet/worksheetStore";
import {
  renameResource,
  deleteResource,
} from "@/features/application/resource/resourceActions";
import { openDatabaseEditorWindow } from "@/features/application/window";
import { safeDataTypeColor, safeDataTypeDisplay } from "./sidebarUtils";
import {
  buildSidebarContextMenuSections,
  useSidebarContextMenu,
  type GraphResourceType,
} from "./sidebarContextMenu";
import {
  SidebarCollapsibleSection,
  SidebarEmptyPlaceholder,
  SidebarListItem,
  SidebarRowActionButton,
  sidebarItemIndent,
  sidebarItemLabelClass,
  sidebarItemRowClass,
} from "./sidebarUi";
import { workbenchPanelHeaderClass, workbenchPanelHeaderTitleClass } from "./workbenchPanelHeaderStyles";
import { SidebarNodesPanel } from "./SidebarNodesPanel";

const Sidebar = forwardRef<HTMLDivElement>((_, ref) => {
  const { t } = useTranslation();
  useContext(GroupContext);
  const sidebarNode = useLayoutStore((s) => s.nodes["sidebar"]);
  const currentTab = sidebarNode?.data?.currentTab as "graphs" | "nodes" | "variables" | "data" | "commands" | "charts" | null;

  const {
    variables,
    setDetailFocus,
    addVariable,
    functions,
    events,
    dataframes,
    triggerImportData,
    openGraph,
    addWorksheet,
    openWorksheet,
    addEvent,
    addFunction,
    deleteEvent,
    deleteFunction,
    createGraph,
    renameGraph,
    duplicateGraph,
  } = useEditorGroup();

  const detailTarget = useDetailTarget();

  const isDetailSelected = (id: string, type: string) =>
    detailTarget != null && "id" in detailTarget && detailTarget.id === id && detailTarget.kind === type;

  const worksheets = useWorksheetStore((s) => s.index);

  const toggleSection = useSidebarStore((s) => s.toggleSection);
  const expandedSections = useSidebarStore((s) => s.expandedSections);
  const isSectionExpanded = useCallback(
    (key: string, defaultExpanded = true) => expandedSections[key] ?? defaultExpanded,
    [expandedSections]
  );

  const listRef = useRef<HTMLDivElement>(null);
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
  const variablesGraphScopeId = useEditorStore((s) => s.variablesGraphScopeId);
  const variablesScopeId = variablesGraphScopeId ?? activeTabId;
  const activeGraphType: GraphResourceType | undefined = activeTabId
    ? (activeTabId in events ? "event" : activeTabId in functions ? "function" : undefined)
    : undefined;
  const variablesScopeGraphType: GraphResourceType | undefined = variablesScopeId
    ? (variablesScopeId in events ? "event" : variablesScopeId in functions ? "function" : undefined)
    : undefined;

  const { variablesGlobal, localVariables } = (() => {
    const global: Record<string, { name: string; dataType?: unknown }> = {};
    const local: Record<string, { name: string; dataType?: unknown }> = {};

    for (const [id, v] of Object.entries(variables)) {
      const scope = (v as { scope?: { type: string; eventId?: string; functionId?: string } }).scope;
      const data = v as { name: string; dataType?: unknown };
      if (scope?.type === "global") {
        global[id] = data;
        continue;
      }
      if (
        variablesScopeId &&
        scope &&
        (scope.eventId === variablesScopeId || scope.functionId === variablesScopeId)
      ) {
        local[id] = data;
      }
    }

    return { variablesGlobal: global, localVariables: local };
  })();

  const eventsCount = Object.keys(events).length;
  const functionsCount = Object.keys(functions).length;
  const variablesCount = Object.keys(variables).length;
  const dataframesCount = Object.keys(dataframes || {}).length;

  const prevCounts = useRef({
    events: eventsCount,
    functions: functionsCount,
    variables: variablesCount,
    dataframes: dataframesCount,
  });

  useEffect(() => {
    const isAdded =
      eventsCount > prevCounts.current.events ||
      functionsCount > prevCounts.current.functions ||
      variablesCount > prevCounts.current.variables ||
      dataframesCount > prevCounts.current.dataframes;

    if (isAdded && listRef.current) {
      listRef.current.scrollTo({ top: listRef.current.scrollHeight, behavior: "smooth" });
    }
    prevCounts.current = {
      events: eventsCount,
      functions: functionsCount,
      variables: variablesCount,
      dataframes: dataframesCount,
    };
  }, [eventsCount, functionsCount, variablesCount, dataframesCount]);

  const createRootEvent = useCallback(() => {
    void addEvent();
  }, [addEvent]);

  const createRootFunction = useCallback(() => {
    void addFunction();
  }, [addFunction]);

  const renameGraphItem = useCallback((id: string, name: string, type: GraphResourceType) => {
    openInputDialog(t("contextMenu.dialog.renameGraphTitle"), name, async (nextName) => {
      await renameGraph(id, nextName, type);
    }, t("contextMenu.dialog.renameSubmit"));
  }, [openInputDialog, renameGraph, t]);

  const deleteGraphItem = useCallback(async (id: string, type: GraphResourceType) => {
    if (type === "event") {
      await deleteEvent(id);
      return;
    }
    await deleteFunction(id);
  }, [deleteEvent, deleteFunction]);

  const duplicateGraphItem = useCallback(async (id: string) => {
    await duplicateGraph(id);
  }, [duplicateGraph]);

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

  const deleteWorksheetItem = useCallback(async (id: string) => {
    await deleteWorksheetWithConfirm(id);
  }, []);

  const renameWorksheetItem = useCallback((id: string, name: string) => {
    openInputDialog(t("contextMenu.dialog.renameWorksheetTitle"), name, async (nextName) => {
      await renameWorksheetResource(id, nextName);
    }, t("contextMenu.dialog.renameSubmit"));
  }, [openInputDialog, t]);

  const revealInExplorer = useCallback(async (request: Parameters<typeof revealProjectResourceInExplorer>[0]) => {
    await revealProjectResourceInExplorer(request);
  }, []);

  const contextMenuSections = buildSidebarContextMenuSections(contextMenu, {
    openGraph,
    createGraph,
    renameGraphItem,
    deleteGraphItem,
    duplicateGraphItem,
    addVariable,
    renameVariableItem,
    deleteVariable: deleteVariableItem,
    openDatabase: openDatabaseEditorWindow,
    renameDatabaseItem,
    deleteDatabaseItem,
    importData: triggerImportData,
    openWorksheet,
    renameWorksheet: renameWorksheetItem,
    deleteWorksheet: deleteWorksheetItem,
    addWorksheet,
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
    extra?: { dataType?: unknown; isGlobal?: boolean },
    readOnly?: boolean,
    onContextMenu?: (e: React.MouseEvent) => void
  ) => {
    const isSelected = isDetailSelected(id, type);
    const dragData = readOnly ? null : buildSidebarDragData(id, name, type, extra as { dataType?: DataType | string } | undefined);

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
        icon={icon}
        label={name}
        onClick={(e) => {
          e.stopPropagation();
          setDetailFocus({ kind: type, id });
          if (type === "event" || type === "function") {
            setVariablesGraphScopeFromResource(id);
          }
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
              <SidebarRowActionButton
                isSelected={isSelected}
                tooltip={t("sidebar.open")}
                onClick={(e) => {
                  e.stopPropagation();
                  openGraph(id, name, type);
                }}
              />
            )}
            {type === "variable" && !readOnly && (
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

  const renderDataItem = (id: string, name: string, data: unknown) => {
    const isLoading = (data as { loading?: unknown }).loading === true;
    const loadError = (data as { loadError?: unknown }).loadError;
    const isSelected = isDetailSelected(id, "data");

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
          setDetailFocus({ kind: "data", id });
        }}
        onDoubleClick={(e) => {
          e.stopPropagation();
          openDatabaseEditorWindow(id);
        }}
        onContextMenu={(e) => openContextMenu(e, { type: "database", id, name })}
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
            <SidebarRowActionButton
              isSelected={isSelected}
              tooltip={t("sidebar.open")}
              onClick={(e) => {
                e.stopPropagation();
                openDatabaseEditorWindow(id);
              }}
            />
          </>
        }
      />
    );
  };

  const renderWorksheetItem = (id: string, name: string) => {
    const isSelected = isDetailSelected(id, "worksheet");
    return (
      <SidebarListItem
        key={id}
        id={id}
        isSelected={isSelected}
        icon={<VscGraphLine size={12} style={{ color: TYPE_ICON_COLORS.worksheet }} />}
        label={name}
        onClick={(e) => {
          e.stopPropagation();
          void openWorksheet(id, name);
        }}
        onDoubleClick={(e) => {
          e.stopPropagation();
          void openWorksheet(id, name);
        }}
        onContextMenu={(e) => openContextMenu(e, { type: "worksheet", id, name })}
        trailing={
          <SidebarRowActionButton
            isSelected={isSelected}
            tooltip={t("sidebar.open")}
            onClick={(e) => {
              e.stopPropagation();
              void openWorksheet(id, name);
            }}
          />
        }
      />
    );
  };

  return (
    <div
      ref={ref}
      className="sidebar-container flex h-full w-full overflow-hidden select-none bg-[var(--sidebar-bg)] relative z-30"
      style={{ pointerEvents: "auto" }}
    >
      <div className="flex flex-col flex-1 min-h-0 bg-[var(--sidebar-bg)]">
        <div className={workbenchPanelHeaderClass}>
          <span className={workbenchPanelHeaderTitleClass}>
            {currentTab === "graphs"
              ? t("activityBar.graphs")
              : currentTab === "nodes"
                ? t("activityBar.nodes")
                : currentTab === "variables"
                ? t("activityBar.variables")
                : currentTab === "data"
                  ? t("activityBar.data")
                  : currentTab === "commands"
                    ? t("activityBar.commands")
                    : currentTab === "charts"
                      ? t("activityBar.charts")
                      : ""}
          </span>
        </div>

        <div className="flex flex-col flex-1 min-h-0 overflow-hidden p-0">
          {currentTab === "graphs" && (
            <div ref={listRef} className="flex flex-col flex-1 min-h-0">
              <SidebarCollapsibleSection
                label={t("sidebar.sections.event")}
                expanded={isSectionExpanded("graphsEvent")}
                onToggle={() => toggleSection("graphsEvent")}
                onAdd={createRootEvent}
                onHeaderContextMenu={(e) => openContextMenu(e, { type: "section", graphType: "event" })}
                onContentContextMenu={(e) => openContextMenu(e, { type: "section", graphType: "event" })}
              >
                {Object.entries(events as Record<string, { name: string }>).map(([id, data]) =>
                  renderItem(id, data.name, "event", undefined, false, (e) =>
                    openContextMenu(e, { type: "graph", id, name: data.name, graphType: "event" })
                  )
                )}
                {Object.keys(events).length === 0 && (
                  <SidebarEmptyPlaceholder>{t("sidebar.noEvents")}</SidebarEmptyPlaceholder>
                )}
              </SidebarCollapsibleSection>

              <SidebarCollapsibleSection
                label={t("sidebar.sections.function")}
                expanded={isSectionExpanded("graphsFunction")}
                onToggle={() => toggleSection("graphsFunction")}
                onAdd={createRootFunction}
                onHeaderContextMenu={(e) => openContextMenu(e, { type: "section", graphType: "function" })}
                onContentContextMenu={(e) => openContextMenu(e, { type: "section", graphType: "function" })}
              >
                {Object.entries(functions as Record<string, { name: string }>).map(([id, data]) =>
                  renderItem(id, data.name, "function", undefined, false, (e) =>
                    openContextMenu(e, { type: "graph", id, name: data.name, graphType: "function" })
                  )
                )}
                {Object.keys(functions).length === 0 && (
                  <SidebarEmptyPlaceholder>{t("sidebar.noFunctions")}</SidebarEmptyPlaceholder>
                )}
              </SidebarCollapsibleSection>
            </div>
          )}

          {currentTab === "nodes" && (
            <div ref={listRef} className="flex min-h-0 flex-1 flex-col">
              <SidebarNodesPanel />
            </div>
          )}

          {currentTab === "variables" && (
            <div ref={listRef} className="flex flex-col flex-1 min-h-0">
              <SidebarCollapsibleSection
                label={t("sidebar.sections.local")}
                expanded={isSectionExpanded("variablesLocal")}
                onToggle={() => toggleSection("variablesLocal")}
                onAdd={variablesScopeGraphType ? () => addVariable(DEFAULT_VARIABLE_NAME, "Int64", false) : undefined}
                onHeaderContextMenu={(e) => openContextMenu(e, { type: "variableSection", isGlobal: false })}
                onContentContextMenu={(e) => openContextMenu(e, { type: "variableSection", isGlobal: false })}
              >
                {Object.entries(localVariables).map(([id, data]: [string, { name: string }]) =>
                  renderItem(id, data.name, "variable", { ...data, isGlobal: false }, false, (e) =>
                    openVariableContextMenu(e, id, data.name)
                  )
                )}
                {!variablesScopeGraphType && (
                  <SidebarEmptyPlaceholder>{t("sidebar.noActiveGraph")}</SidebarEmptyPlaceholder>
                )}
                {variablesScopeGraphType && Object.keys(localVariables).length === 0 && (
                  <SidebarEmptyPlaceholder>—</SidebarEmptyPlaceholder>
                )}
              </SidebarCollapsibleSection>

              <SidebarCollapsibleSection
                label={t("sidebar.sections.global")}
                expanded={isSectionExpanded("variablesGlobal")}
                onToggle={() => toggleSection("variablesGlobal")}
                onAdd={() => addVariable(DEFAULT_VARIABLE_NAME, "Int64", true)}
                onHeaderContextMenu={(e) => openContextMenu(e, { type: "variableSection", isGlobal: true })}
                onContentContextMenu={(e) => openContextMenu(e, { type: "variableSection", isGlobal: true })}
              >
                  {Object.entries(variablesGlobal).map(([id, data]: [string, { name: string }]) =>
                    renderItem(id, data.name, "variable", { ...data, isGlobal: true }, false, (e) =>
                      openVariableContextMenu(e, id, data.name)
                    )
                  )}
                {Object.keys(variablesGlobal).length === 0 && (
                  <SidebarEmptyPlaceholder>—</SidebarEmptyPlaceholder>
                )}
              </SidebarCollapsibleSection>
            </div>
          )}

          {currentTab === "data" && (
            <div className="flex flex-col flex-1 min-h-0">
              <SidebarCollapsibleSection
                label={t("sidebar.sections.data")}
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
                    <SidebarEmptyPlaceholder>{t("sidebar.noData")}</SidebarEmptyPlaceholder>
                  )}
              </SidebarCollapsibleSection>
            </div>
          )}
          {currentTab === "commands" && (
            <CommandsPanel activeTabId={activeTabId} />
          )}
          {currentTab === "charts" && (
            <div ref={listRef} className="flex flex-col flex-1 min-h-0">
              <SidebarCollapsibleSection
                label={t("chartsSidebar.worksheets")}
                expanded={isSectionExpanded("chartsWorksheets")}
                onToggle={() => toggleSection("chartsWorksheets")}
                onAdd={() => void addWorksheet()}
                onHeaderContextMenu={(e) => openContextMenu(e, { type: "worksheetSection" })}
                onContentContextMenu={(e) => openContextMenu(e, { type: "worksheetSection" })}
              >
                {worksheets.map((ws) => renderWorksheetItem(ws.id, ws.name))}
                {worksheets.length === 0 && (
                  <SidebarEmptyPlaceholder>{t("chartsSidebar.noWorksheets")}</SidebarEmptyPlaceholder>
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
        <SidebarEmptyPlaceholder className="py-3">{t("sidebar.noActiveGraph")}</SidebarEmptyPlaceholder>
      </div>
    );
  }

  const reversedUndo = [...undoStack].reverse();

  return (
    <div className="flex flex-col flex-1 min-h-0">
      <SidebarCollapsibleSection
        collapsible={false}
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
            <span className={sidebarItemLabelClass()}>
              {t(`sidebar.commands.${entry.commandType}`, { defaultValue: COMMAND_LABELS[entry.commandType] ?? entry.commandType })}
            </span>
            <span className="shrink-0 text-[10px] text-muted-foreground/70">{formatTime(entry.timestamp)}</span>
          </div>
        )) : (
          <SidebarEmptyPlaceholder>—</SidebarEmptyPlaceholder>
        )}
      </SidebarCollapsibleSection>

      <SidebarCollapsibleSection
        collapsible={false}
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
            <VscRedo size={11} className="shrink-0 text-muted-foreground" />
            <span className={sidebarItemLabelClass()}>
              {t(`sidebar.commands.${entry.commandType}`, { defaultValue: COMMAND_LABELS[entry.commandType] ?? entry.commandType })}
            </span>
            <span className="shrink-0 text-[10px] text-muted-foreground/70">{formatTime(entry.timestamp)}</span>
          </div>
        )) : (
          <SidebarEmptyPlaceholder>—</SidebarEmptyPlaceholder>
        )}
      </SidebarCollapsibleSection>
    </div>
  );
}

export default Sidebar;
