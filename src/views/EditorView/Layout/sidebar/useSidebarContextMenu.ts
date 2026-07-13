import { useCallback, useState } from "react";
import { useTranslation } from "react-i18next";
import { uiStore } from "@/features/core/ui/UIStore";
import { usePositionedContextMenu } from "@/shared/ui/contextMenu";
import type { SidebarContextMenuTarget, SidebarInputDialogState } from "../sidebarContextMenu/sidebarContextMenuTypes";

export function useSidebarContextMenu() {
  const { t } = useTranslation();
  const {
    contextMenu,
    openContextMenu,
    closeContextMenu,
  } = usePositionedContextMenu<SidebarContextMenuTarget>();
  const [inputDialog, setInputDialog] = useState<SidebarInputDialogState | null>(null);

  const openInputDialog = useCallback((
    title: string,
    value: string,
    onSubmit: SidebarInputDialogState["onSubmit"],
    submitLabel?: string,
  ) => {
    setInputDialog({ title, value, onSubmit, submitLabel, error: null });
  }, []);

  const submitInputDialog = useCallback(async () => {
    if (!inputDialog) return;
    const value = inputDialog.value.trim();
    if (!value) return;
    try {
      await inputDialog.onSubmit(value);
      setInputDialog(null);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setInputDialog((prev) => (prev ? { ...prev, error: message } : null));
      uiStore.showToast(message, "error");
    }
  }, [inputDialog]);

  const cancelInputDialog = useCallback(() => {
    setInputDialog(null);
  }, []);

  const updateInputDialogValue = useCallback((value: string) => {
    setInputDialog((prev) => (prev ? { ...prev, value, error: null } : null));
  }, []);

  return {
    contextMenu,
    openContextMenu,
    closeContextMenu,
    inputDialog,
    openInputDialog,
    submitInputDialog,
    cancelInputDialog,
    updateInputDialogValue,
    cancelLabel: t("common.cancel"),
  };
}
