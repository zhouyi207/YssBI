import { confirmDirtyEditorClose } from "./confirmDirtyEditorClose";
import { useEffect } from "react";
import { currentAppWindow } from "@/services/platform/appWindow";
import { workbenchLayoutController } from "@/modules/workbench/public";
import { showWorkbenchLayoutError } from "@/modules/workbench/public";
import {
  captureProjectLifecycleState,
  isProjectLifecycleStateCurrent,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { logger } from "@/utils/frontendLogger";

/** Flushes layout and protects dirty documents before the workbench window closes. */
export function useWorkbenchWindowCloseGuard(): void {
  useEffect(() => {
    const appWindow = currentAppWindow();
    let cancelled = false;
    let unlistenClose: (() => void) | null = null;
    let closePermit: (() => boolean) | null = null;
    let inFlight = false;

    const setupCloseListener = async () => {
      try {
        const subscription = await appWindow.onCloseRequested(async () => {
          if (closePermit) {
            const permit = closePermit;
            closePermit = null;
            if (permit()) return "allow";
            inFlight = false;
            return "prevent";
          }
          if (cancelled) return "prevent";

          if (inFlight) return "prevent";
          inFlight = true;
          const identity = captureProjectLifecycleState();
          const isCurrent = () => !cancelled && isProjectLifecycleStateCurrent(identity);

          if (!(await confirmDirtyEditorClose(isCurrent)) || !isCurrent()) {
            inFlight = false;
            return "prevent";
          }

          try {
            await workbenchLayoutController.flushBeforeWindowClose();
          } catch (error) {
            inFlight = false;
            if (isCurrent()) showWorkbenchLayoutError(error);
            return "prevent";
          }
          if (!isCurrent()) {
            inFlight = false;
            return "prevent";
          }

          closePermit = isCurrent;
          const closeResult = await appWindow.close();
          if (!closeResult.ok) {
            const current = isCurrent();
            if (closePermit === isCurrent || current) {
              closePermit = null;
              inFlight = false;
            }
            if (current) {
              logger.app.error("window close after confirmation failed", "WorkbenchWindow");
            }
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
        if (!cancelled) {
          logger.app.warn("workbench window close guard unavailable", "WorkbenchWindow");
        }
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
