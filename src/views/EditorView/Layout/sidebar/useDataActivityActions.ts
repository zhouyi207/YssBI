import { useCallback } from "react";
import { useTranslation } from "react-i18next";

import { useEditorSessionCommandsContext } from "@/features/application/editor";
import { renameResource } from "@/features/application/resource/resourceActions";
import { revealProjectResourceInExplorer } from "@/features/application/sidebar/sidebarResourceActions";
import { openDatabaseEditorWindow } from "@/features/application/window";
import { useDatabaseRead } from "@/features/core/database/read";
import { ui } from "@/features/core/ui/ui";

type OpenInputDialog = (
  title: string,
  value: string,
  onSubmit: (value: string) => void | Promise<void>,
  submitLabel?: string,
) => void;

export function useDataActivityActions(openInputDialog: OpenInputDialog) {
  const { t } = useTranslation();
  const databases = useDatabaseRead((snapshot) => snapshot.databases);
  const { deleteDataFrame, triggerImportData } = useEditorSessionCommandsContext();

  const renameDatabaseItem = useCallback(
    (id: string, name: string) => {
      openInputDialog(
        t("contextMenu.dialog.renameDataTitle"),
        name,
        async (nextName) => {
          await renameResource({ id, kind: "database" }, nextName);
        },
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
      if (!confirmed) return;
      await deleteDataFrame(id);
    },
    [deleteDataFrame, t],
  );

  const revealInExplorer = useCallback(
    async (request: Parameters<typeof revealProjectResourceInExplorer>[0]) => {
      await revealProjectResourceInExplorer(request);
    },
    [],
  );

  const resolveDatabaseName = useCallback(
    (id: string, fallback: string) => databases[id]?.name ?? fallback,
    [databases],
  );

  return {
    renameDatabaseItem,
    deleteDatabaseItem,
    revealInExplorer,
    resolveDatabaseName,
    triggerImportData,
    openDatabaseEditorWindow,
  };
}
