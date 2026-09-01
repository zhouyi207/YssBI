import { useCallback, useMemo, type MouseEvent } from "react";
import { useTranslation } from "react-i18next";

import { useEditorSessionResources } from "@/features/application/editor";
import { ActionMenu } from "@/shared/ui/actionMenu";
import type { RootDockviewPanelComponent } from "../../RootDockviewHost";
import { SidebarRenameDialog } from "../../sidebar/SidebarRenameDialog";
import { SidebarDataTab } from "../../sidebar/tabs/SidebarDataTab";
import { useDataActivityActions } from "../../sidebar/useDataActivityActions";
import {
  buildDataSidebarContextMenuSections,
  useSidebarContextMenu,
  type DataSidebarContextMenuTarget,
} from "../../sidebarContextMenu";
import { ActivityPanelShell } from "../ActivityPanelShell";

function DataActivityPanelController() {
  const { t } = useTranslation();
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
  } = useSidebarContextMenu<DataSidebarContextMenuTarget>();
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
