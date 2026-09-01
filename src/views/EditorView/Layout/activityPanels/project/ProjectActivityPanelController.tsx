import { useCallback, useMemo, type MouseEvent } from "react";
import { useTranslation } from "react-i18next";

import {
  PROJECT_TREE_CATEGORY_IDS,
  type ProjectTreeCategoryId,
} from "@/features/core/sidebar/projectTreeState";
import { ActionMenu } from "@/shared/ui/actionMenu";
import type { RootDockviewPanelComponent } from "../../RootDockviewHost";
import { SidebarRenameDialog } from "../../sidebar/SidebarRenameDialog";
import type { SidebarProjectTreeActions } from "../../sidebar/rows/SidebarProjectTreeRow";
import { SidebarProjectTab } from "../../sidebar/tabs/SidebarProjectTab";
import { useProjectActivityActions } from "../../sidebar/useProjectActivityActions";
import {
  buildProjectSidebarContextMenuSections,
  useSidebarContextMenu,
  type GraphResourceType,
  type ProjectSidebarContextMenuTarget,
} from "../../sidebarContextMenu";
import { ActivityPanelShell } from "../ActivityPanelShell";

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
  } = useSidebarContextMenu<ProjectSidebarContextMenuTarget>();
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
          addVariable: actions.addVariable,
          renameVariableItem: actions.renameVariableItem,
          deleteVariable: actions.deleteVariableItem,
          promoteVariable: actions.promoteVariable,
          demoteVariable: actions.demoteVariable,
          canDemoteVariable: actions.canDemoteVariable,
          openChart: actions.openChart,
          renameChartItem: actions.renameChartItem,
          duplicateChart: actions.duplicateChart,
          deleteChart: actions.deleteChartItem,
          addChart: actions.addChart,
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

  const openVariableContextMenu = useCallback(
    (event: MouseEvent, id: string, name: string) => {
      openActionMenu(event, actions.openVariableContextMenuTarget(id, name));
    },
    [actions, openActionMenu],
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
      onAddChart: () => void actions.addChart(),
      onAddVariable: (isGlobal) => void actions.addVariable(undefined, "Int64", isGlobal),
      onCategoryContextMenu: openProjectCategoryContextMenu,
      onGraphContextMenu: openGraphContextMenu,
      onVariableContextMenu: openVariableContextMenu,
      onChartContextMenu: openChartContextMenu,
      onOpenChart: actions.openChart,
    }),
    [
      actions,
      openChartContextMenu,
      openGraphContextMenu,
      openProjectCategoryContextMenu,
      openVariableContextMenu,
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
