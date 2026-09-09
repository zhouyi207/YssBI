import { useCallback, useMemo, type MouseEvent } from "react";
import { useTranslation } from "react-i18next";

import {
  ActivityPanelShell,
  SidebarRenameDialog,
  useSidebarContextMenu,
  type RootDockviewPanelComponent,
} from "@/modules/workbench/public";
import {
  PROJECT_TREE_CATEGORY_IDS,
  type ProjectTreeCategoryId,
} from "@/features/core/sidebar/projectTreeState";
import { ActionMenu } from "@/shared/ui/actionMenu";
import { formatInlineUserError } from "@/features/application/userErrorSummary";
import { buildProjectSidebarContextMenuSections } from "./buildProjectSidebarContextMenuSections";
import type { GraphResourceType, ProjectSidebarContextMenuTarget } from "./projectSidebarTypes";
import type { SidebarProjectTreeActions } from "./SidebarProjectTreeRow";
import { SidebarProjectTab } from "./SidebarProjectTab";
import { useProjectActivityActions } from "./useProjectActivityActions";

function ProjectActivityPanelController() {
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
  } = useSidebarContextMenu<ProjectSidebarContextMenuTarget>(formatInlineUserError);
  const actions = useProjectActivityActions(openInputDialog);

  const contextMenuSections = useMemo(
    () =>
      buildProjectSidebarContextMenuSections(
        contextMenu,
        {
          openGraph: actions.openGraph,
          createGraph: actions.createGraph,
          renameGraphItem: actions.renameGraphItem,
          deleteGraphItem: actions.deleteGraphItem,
          duplicateGraphItem: actions.duplicateGraphItem,
          openChart: actions.openChart,
          renameChartItem: actions.renameChartItem,
          duplicateChart: actions.duplicateChart,
          deleteChart: actions.deleteChartItem,
          addChart: actions.addChart,
          openDatabase: actions.openDatabaseEditorWindow,
          renameDatabaseItem: actions.renameDatabaseItem,
          deleteDatabaseItem: actions.deleteDatabaseItem,
          importData: actions.triggerImportData,
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
    ) => openActionMenu(event, target),
    [openActionMenu],
  );

  const openChartContextMenu = useCallback(
    (event: MouseEvent, chartPath: string, name: string) => {
      openActionMenu(event, { type: "chart", chartPath, name });
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
        case PROJECT_TREE_CATEGORY_IDS.charts:
          openActionMenu(event, { type: "chartSection" });
          return;
        case PROJECT_TREE_CATEGORY_IDS.data:
          openActionMenu(event, { type: "dataSection" });
          return;
      }
    },
    [openActionMenu],
  );

  const projectTreeActions = useMemo<SidebarProjectTreeActions>(
    () => ({
      onAddEvent: () => void actions.addEvent(),
      onAddFunction: () => void actions.addFunction(),
      onAddChart: () => void actions.addChart(),
      onImportData: actions.triggerImportData,
      onCategoryContextMenu: openProjectCategoryContextMenu,
      onGraphContextMenu: openGraphContextMenu,
      onChartContextMenu: openChartContextMenu,
      onOpenChart: actions.openChart,
      onDatabaseContextMenu: (event, id, name) =>
        openActionMenu(event, { type: "database", id, name }),
    }),
    [
      actions,
      openActionMenu,
      openChartContextMenu,
      openGraphContextMenu,
      openProjectCategoryContextMenu,
    ],
  );

  return (
    <>
      <ActivityPanelShell>
        <SidebarProjectTab actions={projectTreeActions} />
      </ActivityPanelShell>
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
    </>
  );
}

export const projectActivityPanelContribution: RootDockviewPanelComponent =
  ProjectActivityPanelController;
