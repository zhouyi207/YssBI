import i18n from "i18next";

import { saveFileResource } from "@/features/application/resource/resourceActions";
import { logger } from "@/features/application/observability/appLogger";

import { showBlockingIpcError } from "./blockingErrorDialog";
import { collectDirtyEditorPanels } from "./editorPanelDirty";
import { settleEditorFileEdits } from "./settleEditorFileEdits";
import {
  captureProjectLifecycleState,
  isProjectLifecycleStateCurrent,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";

async function settleBeforeSaving(): Promise<boolean> {
  try {
    await settleEditorFileEdits();
    return true;
  } catch (error) {
    showBlockingIpcError(error, (code) =>
      i18n.t("notifications.project.saveFailed", { error: code }),
    );
    return false;
  }
}

/** Persist every dirty document currently projected by canonical editor metadata. */
export async function saveAllDirtyDocuments(): Promise<boolean> {
  const identity = captureProjectLifecycleState();
  if (!(await settleBeforeSaving())) return false;
  if (!isProjectLifecycleStateCurrent(identity)) return false;
  const dirty = collectDirtyEditorPanels();
  if (dirty.length === 0) return true;

  for (const document of dirty) {
    if (!isProjectLifecycleStateCurrent(identity)) return false;
    try {
      const saved = await saveFileResource(document.resourceRef, document.resourceKind);
      if (!saved || !isProjectLifecycleStateCurrent(identity)) return false;
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      logger.app.error(
        `Failed to save document '${document.title}' (${document.resourceRef}): ${message}`,
        "saveAllDirtyDocuments",
      );
      showBlockingIpcError(error, (code) =>
        i18n.t("notifications.editor.documentSaveFailed", {
          title: document.title,
          error: code,
        }),
      );
      return false;
    }
  }
  if (!(await settleBeforeSaving())) return false;
  return isProjectLifecycleStateCurrent(identity) && collectDirtyEditorPanels().length === 0;
}
