import { FILE_RESOURCE_KINDS } from "@/shared/types/domain/resource";
import { fileResourceHandlers } from "@/features/application/resource/resourceActions";
import { useCallback, useMemo, type MouseEvent } from "react";
import { useTranslation } from "react-i18next";

import {
  SidebarRenameDialog,
  useSidebarContextMenu,
  type RootPanelComponent,
} from "@/modules/workbench/public";
import {
  PROJECT_TREE_CATEGORY_IDS,
  type ProjectTreeCategoryId,
} from "@/features/core/sidebar/projectTreeState";
import { ActionMenu } from "@/shared/ui/actionMenu";
import { formatInlineUserError } from "@/features/application/userErrorSummary";
import { buildProjectSidebarContextMenuSections } from "./buildProjectSidebarContextMenuSections";
import type { ProjectSidebarContextMenuTarget } from "./projectSidebarTypes";
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
          createFile: actions.createFile,
          openFile: actions.openFile,
          renameFile: actions.renameFile,
          duplicateFile: actions.duplicateFile,
          deleteFile: actions.deleteFile,
          openDatabase: actions.openDatabaseInEditor,
          renameDatabaseItem: actions.renameDatabaseItem,
          deleteDatabaseItem: actions.deleteDatabaseItem,
          importData: actions.triggerImportData,
          revealInExplorer: actions.revealInExplorer,
        },
        t,
      ),
    [actions, contextMenu, t],
  );

  const openProjectCategoryContextMenu = useCallback(
    (event: MouseEvent, categoryId: ProjectTreeCategoryId) => {
      const kind = FILE_RESOURCE_KINDS.find(
        (kind) => fileResourceHandlers[kind].categoryId === categoryId,
      );
      if (kind) openActionMenu(event, { type: "fileSection", kind });
      else if (categoryId === PROJECT_TREE_CATEGORY_IDS.data)
        openActionMenu(event, { type: "dataSection" });
    },
    [openActionMenu],
  );

  const projectTreeActions = useMemo<SidebarProjectTreeActions>(
    () => ({
      onCreateFile: (kind) => void actions.createFile(kind),
      onOpenFile: actions.openFile,
      onFileContextMenu: (event, ref) => openActionMenu(event, { type: "file", ...ref }),
      onImportData: actions.triggerImportData,
      onCategoryContextMenu: openProjectCategoryContextMenu,
      onDatabaseContextMenu: (event, id, name) =>
        openActionMenu(event, { type: "database", id, name }),
    }),
    [actions, openActionMenu, openProjectCategoryContextMenu],
  );

  return (
    <>
      <SidebarProjectTab actions={projectTreeActions} />
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

export const projectActivityPanelContribution: RootPanelComponent = ProjectActivityPanelController;
