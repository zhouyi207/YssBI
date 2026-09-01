import { useCallback, useState } from "react";
import { useTranslation } from "react-i18next";
import { usePositionedActionMenu } from "@/shared/ui/actionMenu";
import { formatInlineUserError } from "@/features/application/userErrorSummary";
import type { SidebarInputDialogState } from "./sidebarInputDialog";

export function useSidebarContextMenu<TTarget>() {
  const { t } = useTranslation();
  const { contextMenu, openActionMenu, closeActionMenu } = usePositionedActionMenu<TTarget>();
  const [inputDialog, setInputDialog] = useState<SidebarInputDialogState | null>(null);

  const openInputDialog = useCallback(
    (
      title: string,
      value: string,
      onSubmit: SidebarInputDialogState["onSubmit"],
      submitLabel?: string,
    ) => {
      setInputDialog({ title, value, onSubmit, submitLabel, error: null });
    },
    [],
  );

  const submitInputDialog = useCallback(async () => {
    if (!inputDialog) return;
    const value = inputDialog.value.trim();
    if (!value) return;
    try {
      await inputDialog.onSubmit(value);
      setInputDialog(null);
    } catch (error) {
      const message = t("notifications.sidebar.actionFailed", {
        error: formatInlineUserError(error, t),
      });
      setInputDialog((prev) => (prev ? { ...prev, error: message } : null));
    }
  }, [inputDialog, t]);

  const cancelInputDialog = useCallback(() => {
    setInputDialog(null);
  }, []);

  const updateInputDialogValue = useCallback((value: string) => {
    setInputDialog((prev) => (prev ? { ...prev, value, error: null } : null));
  }, []);

  return {
    contextMenu,
    openActionMenu,
    closeActionMenu,
    inputDialog,
    openInputDialog,
    submitInputDialog,
    cancelInputDialog,
    updateInputDialogValue,
    cancelLabel: t("common.cancel"),
  };
}
