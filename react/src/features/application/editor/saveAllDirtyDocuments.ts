import i18n from "i18next";

import { saveFileResource } from "@/features/application/resource/resourceActions";
import { logger } from "@/utils/frontendLogger";
import { formatApplicationIpcError } from "@/features/application/errorReference";

import { showBlockingIpcError } from "./blockingErrorDialog";
import { collectDirtyEditorPanels } from "./editorPanelDirty";
import { settleEditorFileEdits } from "./settleEditorFileEdits";
import {
  captureProjectLifecycleState,
  isProjectLifecycleStateCurrent,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";

async function settleBeforeSaving(isCurrent: () => boolean): Promise<boolean> {
  if (!isCurrent()) return false;
  try {
    await settleEditorFileEdits();
    return isCurrent();
  } catch (error) {
    if (isCurrent()) {
      showBlockingIpcError(error, (code) =>
        i18n.t("notifications.project.saveFailed", { error: code }),
      );
    }
    return false;
  }
}

/** Persist every dirty document currently projected by canonical editor metadata. */
export async function saveAllDirtyDocuments(
  isActive: () => boolean = () => true,
): Promise<boolean> {
  const identity = captureProjectLifecycleState();
  const isCurrent = () => isActive() && isProjectLifecycleStateCurrent(identity);
  if (!(await settleBeforeSaving(isCurrent))) return false;
  if (!isCurrent()) return false;
  const dirty = collectDirtyEditorPanels();
  if (dirty.length === 0) return true;

  for (const document of dirty) {
    if (!isCurrent()) return false;
    try {
      const saved = await saveFileResource(document.resourceRef, document.resourceKind);
      if (!saved || !isCurrent()) return false;
    } catch (error) {
      if (!isCurrent()) return false;
      logger.app.error(
        `Failed to save document '${document.title}' (${document.resourceRef}): ${formatApplicationIpcError(error)}`,
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
  if (!(await settleBeforeSaving(isCurrent))) return false;
  return isCurrent() && collectDirtyEditorPanels().length === 0;
}
