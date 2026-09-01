import { useEffect } from "react";
import { i18n } from "@/app/i18n";
import { currentAppWindow } from "@/services/platform/appWindow";
import { workbenchLayoutController } from "@/features/application/layout/workbenchLayoutController";
import { showWorkbenchLayoutError } from "@/features/application/layout/workbenchLayoutErrorFeedback";
import { collectDirtyEditorPanels } from "@/features/core/layout/editorPanelDirty";
import { uiStore } from "@/features/core/ui/UIStore";
import { logger } from "@/features/application/observability/appLogger";
import { saveAllDirtyGraphs } from "./saveAllDirtyGraphs";

/** Flushes layout and protects dirty documents before the workbench window closes. */
export function useWorkbenchWindowCloseGuard(): void {
  useEffect(() => {
    const appWindow = currentAppWindow();
    let cancelled = false;
    let unlistenClose: (() => void) | null = null;
    let allowDestructiveClose = false;
    let inFlight = false;

    const setupCloseListener = async () => {
      try {
        const subscription = await appWindow.onCloseRequested(async () => {
          if (allowDestructiveClose) {
            allowDestructiveClose = false;
            return "allow";
          }

          if (inFlight) return "prevent";
          inFlight = true;

          const dirty = collectDirtyEditorPanels();
          if (dirty.length > 0) {
            const titles = dirty.map((tab) => `• ${tab.title}`).join("\n");
            const choice = await uiStore.confirm3({
              title: i18n.t("editor.unsavedTitle", { defaultValue: "保存更改？" }),
              message: i18n.t("editor.unsavedMessage", {
                defaultValue: `以下 {{count}} 个图存在未保存修改：\n{{titles}}\n\n关闭前是否保存？`,
                count: dirty.length,
                titles,
              }),
              confirmText: i18n.t("editor.unsavedSaveAll", { defaultValue: "全部保存" }),
              discardText: i18n.t("editor.unsavedDiscard", { defaultValue: "不保存" }),
              cancelText: i18n.t("common.cancel", { defaultValue: "取消" }),
              type: "info",
            });

            if (choice === "cancel") {
              inFlight = false;
              return "prevent";
            }

            if (choice === "confirm") {
              const saved = await saveAllDirtyGraphs();
              if (!saved) {
                inFlight = false;
                return "prevent";
              }
            }
          }

          try {
            await workbenchLayoutController.flushBeforeWindowClose();
          } catch (error) {
            inFlight = false;
            showWorkbenchLayoutError(error);
            return "prevent";
          }

          allowDestructiveClose = true;
          const closeResult = await appWindow.close();
          if (!closeResult.ok) {
            logger.app.error("window close after confirmation failed", "WorkbenchWindow");
            allowDestructiveClose = false;
            inFlight = false;
          }
          return "prevent";
        });

        if (cancelled) {
          if (subscription.ok) subscription.value();
        } else if (subscription.ok) {
          unlistenClose = subscription.value;
        } else {
          logger.app.warn("workbench window close guard unavailable", "WorkbenchWindow");
        }
      } catch {
        logger.app.warn("workbench window close guard unavailable", "WorkbenchWindow");
      }
    };

    void setupCloseListener();

    return () => {
      cancelled = true;
      unlistenClose?.();
      unlistenClose = null;
    };
  }, []);
}
