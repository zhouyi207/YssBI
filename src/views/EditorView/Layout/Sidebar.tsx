import { useCallback, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { useEditorSessionResources } from '@/features/application/editor';

import { useWorkbenchStore } from '@/features/core/workbench';
import {
  PROJECT_TREE_CATEGORY_IDS,
  type ProjectTreeCategoryId,
} from '@/features/core/sidebar/projectTreeState';
import { ActionMenu } from '@/shared/ui/actionMenu';
import {
  buildSidebarContextMenuSections,
  useSidebarContextMenu,
  type GraphResourceType,
} from './sidebarContextMenu';
import { SidebarNodesTab } from './sidebar/tabs/SidebarNodesTab';
import { SidebarRenameDialog } from './sidebar/SidebarRenameDialog';
import { useSidebarResourceActions } from './sidebar/useSidebarResourceActions';
import { SidebarDataTab } from './sidebar/tabs/SidebarDataTab';
import { SidebarCommandsTab } from './sidebar/tabs/SidebarCommandsTab';
import { SidebarProjectTab } from './sidebar/tabs/SidebarProjectTab';
import type { SidebarProjectTreeActions } from './sidebar/rows/SidebarProjectTreeRow';


function Sidebar() {
  const { t } = useTranslation();

  const currentTab = useWorkbenchStore((state) => state.sidebarCurrentTab);

  const { dataframes } = useEditorSessionResources();

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
      buildSidebarContextMenuSections(contextMenu, {
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
      }, t),
    [actions, contextMenu, t],
  );

  const openGraphContextMenu = useCallback(
    (
      e: React.MouseEvent,
      target: { type: 'graph'; id: string; name: string; graphType: GraphResourceType },
    ) => {
      openActionMenu(e, target);
    },
    [openActionMenu],
  );

  const openVariableContextMenu = useCallback(
    (e: React.MouseEvent, id: string, name: string) => {
      openActionMenu(e, actions.openVariableContextMenuTarget(id, name));
    },
    [actions, openActionMenu],
  );

  const openDatabaseContextMenu = useCallback(
    (e: React.MouseEvent, id: string, name: string) => {
      openActionMenu(e, { type: 'database', id, name: actions.resolveDatabaseName(id, name) });
    },
    [actions, openActionMenu],
  );

  const openDataSectionContextMenu = useCallback(
    (e: React.MouseEvent) => {
      openActionMenu(e, { type: 'dataSection' });
    },
    [openActionMenu],
  );

  const openWorksheetContextMenu = useCallback(
    (e: React.MouseEvent, worksheetPath: string, name: string) => {
      openActionMenu(e, { type: 'worksheet', worksheetPath, name });
    },
    [openActionMenu],
  );

  const openProjectCategoryContextMenu = useCallback(
    (event: React.MouseEvent, categoryId: ProjectTreeCategoryId) => {
      switch (categoryId) {
        case PROJECT_TREE_CATEGORY_IDS.events:
          openActionMenu(event, { type: 'section', graphType: 'event' });
          return;
        case PROJECT_TREE_CATEGORY_IDS.functions:
          openActionMenu(event, { type: 'section', graphType: 'function' });
          return;
        case PROJECT_TREE_CATEGORY_IDS.worksheets:
          openActionMenu(event, { type: 'worksheetSection' });
          return;
        case PROJECT_TREE_CATEGORY_IDS.variables:
          openActionMenu(event, { type: 'variableSection' });
          return;
        case PROJECT_TREE_CATEGORY_IDS.localVariables:
          openActionMenu(event, { type: 'variableSection', isGlobal: false });
          return;
        case PROJECT_TREE_CATEGORY_IDS.globalVariables:
          openActionMenu(event, { type: 'variableSection', isGlobal: true });
          return;
      }
    },
    [openActionMenu],
  );

  const projectTreeActions: SidebarProjectTreeActions = {
    onAddEvent: () => void actions.addEvent(),
    onAddFunction: () => void actions.addFunction(),
    onAddWorksheet: () => void actions.addWorksheet(),
    onAddVariable: (isGlobal) => void actions.addVariable(undefined, 'Int64', isGlobal),
    onCategoryContextMenu: openProjectCategoryContextMenu,
    onGraphContextMenu: openGraphContextMenu,
    onVariableContextMenu: openVariableContextMenu,
    onWorksheetContextMenu: openWorksheetContextMenu,
    onOpenWorksheet: actions.openWorksheet,
  };

  return (
    <div
      className="sidebar-container relative z-30 flex h-full w-full min-w-0 select-none overflow-hidden bg-sidebar"
      style={{ pointerEvents: 'auto' }}
    >
      <div className="flex min-h-0 min-w-0 flex-1 flex-col bg-sidebar">
        <div className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden p-0">
          {currentTab === 'project' && <SidebarProjectTab actions={projectTreeActions} />}

          {currentTab === 'nodes' && <SidebarNodesTab />}

          {currentTab === 'data' && (
            <SidebarDataTab
              dataframes={dataframes ?? {}}
              onImport={actions.triggerImportData}
              onSectionContextMenu={openDataSectionContextMenu}
              onDatabaseContextMenu={openDatabaseContextMenu}
            />
          )}

          {currentTab === 'commands' && <SidebarCommandsTab />}
        </div>
      </div>

      {contextMenu && (
        <ActionMenu
          position={{ x: contextMenu.x, y: contextMenu.y }}
          sections={contextMenuSections}
          onClose={closeActionMenu}
        />
      )}

      <SidebarRenameDialog
        dialog={inputDialog}
        cancelLabel={cancelLabel}
        onCancel={cancelInputDialog}
        onSubmit={() => void submitInputDialog()}
        onValueChange={updateInputDialogValue}
      />
    </div>
  );
}

export default Sidebar;
