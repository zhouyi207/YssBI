import { i18n } from "@/app/i18n";
import { uiStore } from "@/features/core/ui/UIStore";
import { collectDirtyEditorPanels } from "./editorPanelDirty";
import { saveAllDirtyDocuments } from "./saveAllDirtyDocuments";
import { settleEditorFileEdits } from "./settleEditorFileEdits";
import {
  captureProjectLifecycleState,
  isProjectLifecycleStateCurrent,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { showBlockingIpcError } from "./blockingErrorDialog";

/** Shared save/discard/cancel decision for leaving a project or closing its window. */
export async function confirmDirtyEditorClose(
  isActive: () => boolean = () => true,
): Promise<boolean> {
  const identity = captureProjectLifecycleState();
  const isCurrent = () => isActive() && isProjectLifecycleStateCurrent(identity);
  if (!isCurrent()) return false;
  try {
    await settleEditorFileEdits();
  } catch (error) {
    if (isCurrent()) showBlockingIpcError(error, () => i18n.t("editor.close.failed"));
    return false;
  }
  if (!isCurrent()) return false;
  const dirty = collectDirtyEditorPanels();
  if (dirty.length > 0) {
    const titles = dirty.map((tab) => `• ${tab.title}`).join("\n");
    const choice = await uiStore.confirm3({
      title: i18n.t("editor.unsavedTitle", { defaultValue: "保存更改？" }),
      message: i18n.t("editor.unsavedMessage", {
        defaultValue: `以下 {{count}} 个文件存在未保存修改：\n{{titles}}\n\n关闭前是否保存？`,
        count: dirty.length,
        titles,
      }),
      confirmText: i18n.t("editor.unsavedSaveAll", { defaultValue: "全部保存" }),
      discardText: i18n.t("editor.unsavedDiscard", { defaultValue: "不保存" }),
      cancelText: i18n.t("common.cancel", { defaultValue: "取消" }),
      type: "info",
    });

    if (choice === "cancel" || !isCurrent()) {
      return false;
    }

    if (choice === "confirm") {
      const saved = await saveAllDirtyDocuments(isCurrent);
      if (!saved) {
        return false;
      }
    }
  }

  return isCurrent();
}
