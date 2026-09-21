import { confirmDirtyEditorClose } from "./confirmDirtyEditorClose";
import { useEffect } from "react";
import { currentAppWindow } from "@/services/platform/appWindow";
import { workbenchLayoutController } from "@/modules/workbench/public";
import { showWorkbenchLayoutError } from "@/modules/workbench/public";
import { logger } from "@/features/application/observability/appLogger";

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

          if (!(await confirmDirtyEditorClose())) {
            inFlight = false;
            return "prevent";
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
