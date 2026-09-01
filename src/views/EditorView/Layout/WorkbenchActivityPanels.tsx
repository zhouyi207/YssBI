import {
  createContext,
  useCallback,
  useContext,
  useMemo,
  type MouseEvent,
  type ReactNode,
} from "react";
import { useTranslation } from "react-i18next";

import { useEditorSessionResources } from "@/features/application/editor";
import {
  buildSidebarContextMenuSections,
  useSidebarContextMenu,
  type GraphResourceType,
} from "./sidebarContextMenu";
import { useSidebarResourceActions } from "./sidebar/useSidebarResourceActions";
import { SidebarRenameDialog } from "./sidebar/SidebarRenameDialog";
import { SidebarCommandsTab } from "./sidebar/tabs/SidebarCommandsTab";
import { SidebarDataTab } from "./sidebar/tabs/SidebarDataTab";
import { SidebarNodesTab } from "./sidebar/tabs/SidebarNodesTab";
import { SidebarProjectTab } from "./sidebar/tabs/SidebarProjectTab";
import type { SidebarProjectTreeActions } from "./sidebar/rows/SidebarProjectTreeRow";
import { PROJECT_TREE_CATEGORY_IDS } from "@/features/core/sidebar/projectTreeState";
import type { ProjectTreeCategoryId } from "@/features/core/sidebar/projectTreeState";
import { ActionMenu } from "@/shared/ui/actionMenu";

interface WorkbenchActivityPanelsContextValue {
  readonly projectTreeActions: SidebarProjectTreeActions;
  readonly triggerImportData: () => void;
  readonly openDataSectionContextMenu: (event: MouseEvent) => void;
  readonly openDatabaseContextMenu: (event: MouseEvent, id: string, name: string) => void;
}

const WorkbenchActivityPanelsContext = createContext<WorkbenchActivityPanelsContextValue | null>(
  null,
);

type ActivityPanelShellProps = {
  readonly children: ReactNode;
};

function ActivityPanelShell({ children }: ActivityPanelShellProps) {
  return (
    <div
      className="sidebar-container relative z-30 flex h-full w-full min-w-0 select-none overflow-hidden bg-sidebar"
      style={{ pointerEvents: "auto" }}
      data-workbench-activity-panel
    >
      <div className="flex min-h-0 min-w-0 flex-1 flex-col bg-sidebar">
        <div className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden p-0">{children}</div>
      </div>
    </div>
  );
}

function useActivityPanelsContext(): WorkbenchActivityPanelsContextValue {
  const value = useContext(WorkbenchActivityPanelsContext);
  if (!value) {
    throw new Error("Workbench activity panels must be rendered inside their provider");
  }
  return value;
}

export function WorkbenchActivityPanelsProvider({ children }: { children: ReactNode }) {
  const { t } = useTranslation();
  const {
    contextMenu,
    closeActionMenu,
    openActionMenu,
    inputDialog,
    openInputDialog,
    submitInputDialog,
    cancelInputDialog,
    updateInputDialogValue,
    cancelLabel,
  } = useSidebarContextMenu();
  const actions = useSidebarResourceActions(openInputDialog);

  const contextMenuSections = useMemo(
    () =>
      buildSidebarContextMenuSections(
        contextMenu,
        {
          openGraph: actions.openGraph,
          createGraph: actions.createGraph,
          renameGraphItem: actions.renameGraphItem,
          deleteGraphItem: actions.deleteGraphItem,
          duplicateGraphItem: actions.duplicateGraphItem,
          addVariable: actions.addVariable,
          renameVariableItem: actions.renameVariableItem,
          deleteVariable: actions.deleteVariableItem,
          promoteVariable: actions.promoteVariable,
          demoteVariable: actions.demoteVariable,
          canDemoteVariable: actions.canDemoteVariable,
          openDatabase: actions.openDatabaseEditorWindow,
          renameDatabaseItem: actions.renameDatabaseItem,
          deleteDatabaseItem: actions.deleteDatabaseItem,
          importData: actions.triggerImportData,
          openWorksheet: actions.openWorksheet,
          renameWorksheetItem: actions.renameWorksheetItem,
          duplicateWorksheet: actions.duplicateWorksheet,
          deleteWorksheet: actions.deleteWorksheetItem,
          addWorksheet: actions.addWorksheet,
          revealInExplorer: actions.revealInExplorer,
        },
        t,
      ),
    [actions, contextMenu, t],
  );

  const openGraphContextMenu = useCallback(
    (
      event: MouseEvent,
      target: { type: "graph"; id: string; name: string; graphType: GraphResourceType },
    ) => {
      openActionMenu(event, target);
    },
    [openActionMenu],
  );

  const openVariableContextMenu = useCallback(
    (event: MouseEvent, id: string, name: string) => {
      openActionMenu(event, actions.openVariableContextMenuTarget(id, name));
    },
    [actions, openActionMenu],
  );

  const openDatabaseContextMenu = useCallback(
    (event: MouseEvent, id: string, name: string) => {
      openActionMenu(event, {
        type: "database",
        id,
        name: actions.resolveDatabaseName(id, name),
      });
    },
    [actions, openActionMenu],
  );

  const openDataSectionContextMenu = useCallback(
    (event: MouseEvent) => {
      openActionMenu(event, { type: "dataSection" });
    },
    [openActionMenu],
  );

  const openWorksheetContextMenu = useCallback(
    (event: MouseEvent, worksheetPath: string, name: string) => {
      openActionMenu(event, { type: "worksheet", worksheetPath, name });
    },
    [openActionMenu],
  );

  const openProjectCategoryContextMenu = useCallback(
    (event: MouseEvent, categoryId: ProjectTreeCategoryId) => {
      switch (categoryId) {
        case PROJECT_TREE_CATEGORY_IDS.events:
          openActionMenu(event, { type: "section", graphType: "event" });
          return;
        case PROJECT_TREE_CATEGORY_IDS.functions:
          openActionMenu(event, { type: "section", graphType: "function" });
          return;
        case PROJECT_TREE_CATEGORY_IDS.worksheets:
          openActionMenu(event, { type: "worksheetSection" });
          return;
        case PROJECT_TREE_CATEGORY_IDS.variables:
          openActionMenu(event, { type: "variableSection" });
          return;
        case PROJECT_TREE_CATEGORY_IDS.localVariables:
          openActionMenu(event, { type: "variableSection", isGlobal: false });
          return;
        case PROJECT_TREE_CATEGORY_IDS.globalVariables:
          openActionMenu(event, { type: "variableSection", isGlobal: true });
          return;
      }
    },
    [openActionMenu],
  );

  const projectTreeActions = useMemo<SidebarProjectTreeActions>(
    () => ({
      onAddEvent: () => void actions.addEvent(),
      onAddFunction: () => void actions.addFunction(),
      onAddWorksheet: () => void actions.addWorksheet(),
      onAddVariable: (isGlobal) => void actions.addVariable(undefined, "Int64", isGlobal),
      onCategoryContextMenu: openProjectCategoryContextMenu,
      onGraphContextMenu: openGraphContextMenu,
      onVariableContextMenu: openVariableContextMenu,
      onWorksheetContextMenu: openWorksheetContextMenu,
      onOpenWorksheet: actions.openWorksheet,
    }),
    [
      actions,
      openGraphContextMenu,
      openProjectCategoryContextMenu,
      openVariableContextMenu,
      openWorksheetContextMenu,
    ],
  );

  const contextValue = useMemo<WorkbenchActivityPanelsContextValue>(
    () => ({
      projectTreeActions,
      triggerImportData: actions.triggerImportData,
      openDataSectionContextMenu,
      openDatabaseContextMenu,
    }),
    [
      actions.triggerImportData,
      openDataSectionContextMenu,
      openDatabaseContextMenu,
      projectTreeActions,
    ],
  );

  return (
    <WorkbenchActivityPanelsContext.Provider value={contextValue}>
      {children}
      {contextMenu ? (
        <ActionMenu
          position={{ x: contextMenu.x, y: contextMenu.y }}
          sections={contextMenuSections}
          onClose={closeActionMenu}
        />
      ) : null}
      <SidebarRenameDialog
        dialog={inputDialog}
        cancelLabel={cancelLabel}
        onCancel={cancelInputDialog}
        onSubmit={() => void submitInputDialog()}
        onValueChange={updateInputDialogValue}
      />
    </WorkbenchActivityPanelsContext.Provider>
  );
}

export function WorkbenchProjectPanel() {
  const { projectTreeActions } = useActivityPanelsContext();
  return (
    <ActivityPanelShell>
      <SidebarProjectTab actions={projectTreeActions} />
    </ActivityPanelShell>
  );
}

export function WorkbenchNodesPanel() {
  return (
    <ActivityPanelShell>
      <SidebarNodesTab />
    </ActivityPanelShell>
  );
}

export function WorkbenchDataPanel() {
  const { dataframes } = useEditorSessionResources();
  const { triggerImportData, openDataSectionContextMenu, openDatabaseContextMenu } =
    useActivityPanelsContext();
  return (
    <ActivityPanelShell>
      <SidebarDataTab
        dataframes={dataframes ?? {}}
        onImport={triggerImportData}
        onSectionContextMenu={openDataSectionContextMenu}
        onDatabaseContextMenu={openDatabaseContextMenu}
      />
    </ActivityPanelShell>
  );
}

export function WorkbenchCommandsPanel() {
  return (
    <ActivityPanelShell>
      <SidebarCommandsTab />
    </ActivityPanelShell>
  );
}
