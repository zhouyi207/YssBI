import { useCallback, useState } from "react";
import { usePositionedContextMenu } from "@/shared/ui/contextMenu";
import type { SidebarContextMenuTarget, SidebarInputDialogState } from "./sidebarContextMenuTypes";

export function useSidebarContextMenu() {
  const {
    contextMenu,
    setContextMenu,
    openContextMenu,
    closeContextMenu,
  } = usePositionedContextMenu<SidebarContextMenuTarget>();
  const [inputDialog, setInputDialog] = useState<SidebarInputDialogState | null>(null);

  const openInputDialog = useCallback((
    title: string,
    value: string,
    onSubmit: SidebarInputDialogState["onSubmit"],
    submitLabel = "OK"
  ) => {
    setInputDialog({ title, value, onSubmit, submitLabel });
  }, []);

  const submitInputDialog = useCallback(async () => {
    if (!inputDialog) return;
    const value = inputDialog.value.trim();
    if (!value) return;
    await inputDialog.onSubmit(value);
    setInputDialog(null);
  }, [inputDialog]);

  return {
    contextMenu,
    setContextMenu,
    openContextMenu,
    closeContextMenu,
    inputDialog,
    setInputDialog,
    openInputDialog,
    submitInputDialog,
  };
}
