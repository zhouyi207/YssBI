import { useCallback, useMemo, type MouseEvent } from "react";
import { useTranslation } from "react-i18next";

import {
  ActivityPanelShell,
  SidebarRenameDialog,
  useSidebarContextMenu,
  type RootDockviewPanelComponent,
} from "@/modules/workbench/public";
import { useDatabaseRead } from "@/features/core/database/read";
import { formatInlineUserError } from "@/features/application/userErrorSummary";
import { ActionMenu } from "@/shared/ui/actionMenu";
import { buildDataSidebarContextMenuSections } from "./buildDataSidebarContextMenuSections";
import type { DataSidebarContextMenuTarget } from "./dataSidebarTypes";
import { SidebarDataTab } from "./SidebarDataTab";
import { useDataActivityActions } from "./useDataActivityActions";

function DataActivityPanelController() {
  const { t } = useTranslation();
  const dataframes = useDatabaseRead((snapshot) => snapshot.databases);
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
  } = useSidebarContextMenu<DataSidebarContextMenuTarget>(formatInlineUserError);
  const actions = useDataActivityActions(openInputDialog);

  const contextMenuSections = useMemo(
    () =>
      buildDataSidebarContextMenuSections(
        contextMenu,
        {
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
    (event: MouseEvent) => openActionMenu(event, { type: "dataSection" }),
    [openActionMenu],
  );

  return (
    <>
      <ActivityPanelShell>
        <SidebarDataTab
          dataframes={dataframes ?? {}}
          onImport={actions.triggerImportData}
          onSectionContextMenu={openDataSectionContextMenu}
          onDatabaseContextMenu={openDatabaseContextMenu}
        />
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

export const dataActivityPanelContribution: RootDockviewPanelComponent =
  DataActivityPanelController;
