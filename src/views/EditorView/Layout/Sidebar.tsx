import { forwardRef, useCallback, useContext, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { useEditorGroup } from '@/features/application/editor';
import { GroupContext } from '@/features/core/editor';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { ContextMenu } from '@/shared/ui/contextMenu';
import {
  buildSidebarContextMenuSections,
  useSidebarContextMenu,
  type GraphResourceType,
} from './sidebarContextMenu';
import { workbenchPanelHeaderClass, workbenchPanelHeaderTitleClass } from './workbenchPanelHeaderStyles';
import { SidebarNodesTab } from './sidebar/tabs/SidebarNodesTab';
import { SidebarRenameDialog } from './sidebar/SidebarRenameDialog';
import { useSidebarResourceActions } from './sidebar/useSidebarResourceActions';
import { SidebarGraphsTab } from './sidebar/tabs/SidebarGraphsTab';
import { SidebarVariablesTab } from './sidebar/tabs/SidebarVariablesTab';
import { SidebarDataTab } from './sidebar/tabs/SidebarDataTab';
import { SidebarChartsTab } from './sidebar/tabs/SidebarChartsTab';
import { SidebarCommandsTab } from './sidebar/tabs/SidebarCommandsTab';

type SidebarTab = 'graphs' | 'nodes' | 'variables' | 'data' | 'commands' | 'charts';

const TAB_TITLE_KEYS: Record<SidebarTab, string> = {
  graphs: 'activityBar.graphs',
  nodes: 'activityBar.nodes',
  variables: 'activityBar.variables',
  data: 'activityBar.data',
  commands: 'activityBar.commands',
  charts: 'activityBar.charts',
};

const Sidebar = forwardRef<HTMLDivElement>((_, ref) => {
  const { t } = useTranslation();
  useContext(GroupContext);

  const sidebarNode = useLayoutStore((s) => s.nodes['sidebar']);
  const currentTab = (sidebarNode?.data?.currentTab as SidebarTab | null) ?? null;

  const {
    variables,
    functions,
    events,
    dataframes,
  } = useEditorGroup();

  const {
    contextMenu,
    closeContextMenu,
    openContextMenu,
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
        openDatabase: actions.openDatabaseEditorWindow,
        renameDatabaseItem: actions.renameDatabaseItem,
        deleteDatabaseItem: actions.deleteDatabaseItem,
        importData: actions.triggerImportData,
        openWorksheet: actions.openWorksheet,
        renameWorksheet: actions.renameWorksheetItem,
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
      openContextMenu(e, target);
    },
    [openContextMenu],
  );

  const openGraphSectionContextMenu = useCallback(
    (e: React.MouseEvent, target: { type: 'section'; graphType: GraphResourceType }) => {
      openContextMenu(e, target);
    },
    [openContextMenu],
  );

  const openVariableContextMenu = useCallback(
    (e: React.MouseEvent, id: string, name: string) => {
      openContextMenu(e, actions.openVariableContextMenuTarget(id, name));
    },
    [actions, openContextMenu],
  );

  const openVariableSectionContextMenu = useCallback(
    (e: React.MouseEvent, isGlobal: boolean) => {
      openContextMenu(e, { type: 'variableSection', isGlobal });
    },
    [openContextMenu],
  );

  const openDatabaseContextMenu = useCallback(
    (e: React.MouseEvent, id: string, name: string) => {
      openContextMenu(e, { type: 'database', id, name: actions.resolveDatabaseName(id, name) });
    },
    [actions, openContextMenu],
  );

  const openDataSectionContextMenu = useCallback(
    (e: React.MouseEvent) => {
      openContextMenu(e, { type: 'dataSection' });
    },
    [openContextMenu],
  );

  const openWorksheetSectionContextMenu = useCallback(
    (e: React.MouseEvent) => {
      openContextMenu(e, { type: 'worksheetSection' });
    },
    [openContextMenu],
  );

  const openWorksheetContextMenu = useCallback(
    (e: React.MouseEvent, id: string, name: string) => {
      openContextMenu(e, { type: 'worksheet', id, name });
    },
    [openContextMenu],
  );

  return (
    <div
      ref={ref}
      className="sidebar-container relative z-30 flex h-full w-full min-w-0 overflow-hidden select-none bg-[var(--sidebar-bg)]"
      style={{ pointerEvents: 'auto' }}
    >
      <div className="flex min-h-0 min-w-0 flex-1 flex-col bg-[var(--sidebar-bg)]">
        <div className={workbenchPanelHeaderClass}>
          <span className={workbenchPanelHeaderTitleClass}>
            {currentTab ? t(TAB_TITLE_KEYS[currentTab]) : ''}
          </span>
        </div>

        <div className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden p-0">
          {currentTab === 'graphs' && (
            <SidebarGraphsTab
              events={events as Record<string, { name: string }>}
              functions={functions as Record<string, { name: string }>}
              onAddEvent={() => void actions.addEvent()}
              onAddFunction={() => void actions.addFunction()}
              onOpenContextMenu={openGraphSectionContextMenu}
              onGraphContextMenu={openGraphContextMenu}
            />
          )}

          {currentTab === 'nodes' && <SidebarNodesTab />}

          {currentTab === 'variables' && (
            <SidebarVariablesTab
              variables={variables}
              onAddVariable={(name, dataType, isGlobal) => void actions.addVariable(name, dataType, isGlobal)}
              onSectionContextMenu={openVariableSectionContextMenu}
              onVariableContextMenu={openVariableContextMenu}
            />
          )}

          {currentTab === 'data' && (
            <SidebarDataTab
              dataframes={dataframes ?? {}}
              onImport={actions.triggerImportData}
              onSectionContextMenu={openDataSectionContextMenu}
              onDatabaseContextMenu={openDatabaseContextMenu}
            />
          )}

          {currentTab === 'commands' && <SidebarCommandsTab />}

          {currentTab === 'charts' && (
            <SidebarChartsTab
              onAddWorksheet={() => void actions.addWorksheet()}
              onOpenWorksheet={actions.openWorksheet}
              onSectionContextMenu={openWorksheetSectionContextMenu}
              onWorksheetContextMenu={openWorksheetContextMenu}
            />
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

      <SidebarRenameDialog
        dialog={inputDialog}
        cancelLabel={cancelLabel}
        onCancel={cancelInputDialog}
        onSubmit={() => void submitInputDialog()}
        onValueChange={updateInputDialogValue}
      />
    </div>
  );
});

Sidebar.displayName = 'Sidebar';

export default Sidebar;
