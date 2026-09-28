import { useCallback } from "react";
import { useTranslation } from "react-i18next";
import { useDatabaseManagement } from "@/features/application/dataManagement";
import { revealProjectResourceInExplorer } from "@/features/application/sidebar/sidebarResourceActions";
import {
  renameResource,
  type FileResourceRef,
} from "@/features/application/resource/resourceActions";
import {
  duplicateFile,
  deleteFileWithConfirm,
} from "@/features/application/resource/fileManagement";
import { useFileManagement } from "@/features/application/resource/useFileManagement";
import { openFileInEditor } from "@/features/application/editor/openFileInEditor";
import { openDatabaseInEditor } from "@/features/application/editor/openDatabaseInEditor";
import { showBlockingIpcError } from "@/features/application/editor/blockingErrorDialog";
import { isEditorOpenRejectionHandled } from "@/features/application/editor/openEditorPanel";
import { ui } from "@/features/core/ui/ui";

type OpenInputDialog = (
  title: string,
  value: string,
  onSubmit: (value: string) => void | Promise<void>,
  submitLabel?: string,
) => void;
export function useProjectActivityActions(openInputDialog: OpenInputDialog) {
  const { t } = useTranslation();
  const { createFile } = useFileManagement();
  const { deleteDataFrame, triggerImportData } = useDatabaseManagement();
  const perform = (operation: () => Promise<unknown>) => {
    void operation().catch((error) => {
      if (!isEditorOpenRejectionHandled(error))
        showBlockingIpcError(error, (code) => t("documents.operationFailed", { error: code }));
    });
  };
  const renameFile = (ref: FileResourceRef, name: string) =>
    openInputDialog(
      t("documents.rename"),
      name,
      (nextName) => renameResource(ref, nextName),
      t("contextMenu.dialog.renameSubmit"),
    );
  const renameDatabaseItem = useCallback(
    (id: string, name: string) => {
      openInputDialog(
        t("contextMenu.dialog.renameDataTitle"),
        name,
        (nextName) => renameResource({ id, kind: "database" }, nextName),
        t("contextMenu.dialog.renameSubmit"),
      );
    },
    [openInputDialog, t],
  );
  const deleteDatabaseItem = useCallback(
    async (id: string, name: string) => {
      const confirmed = await ui.confirm({
        title: t("sidebar.deleteDataTitle"),
        message: t("sidebar.deleteDataMessage", { name }),
        confirmText: t("contextMenu.sidebar.delete"),
        cancelText: t("common.cancel"),
        type: "danger",
      });
      if (confirmed) await deleteDataFrame(id);
    },
    [deleteDataFrame, t],
  );
  return {
    createFile,
    openFile: (ref: FileResourceRef) => perform(() => openFileInEditor(ref.id, ref.kind)),
    renameFile,
    duplicateFile: (ref: FileResourceRef) => perform(() => duplicateFile(ref)),
    deleteFile: (ref: FileResourceRef) => perform(() => deleteFileWithConfirm(ref)),
    revealInExplorer: (request: Parameters<typeof revealProjectResourceInExplorer>[0]) =>
      perform(() => revealProjectResourceInExplorer(request)),
    renameDatabaseItem,
    deleteDatabaseItem,
    triggerImportData,
    openDatabaseInEditor,
  };
}
