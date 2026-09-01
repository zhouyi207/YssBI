import i18n from "i18next";

import { projectPublicationCoordinator } from "@/features/application/editorMutation/projectPublicationCoordinator";
import {
  captureProjectCommandContext,
  type ProjectCommandContext,
} from "@/features/application/projectCommandContext";

import { uiStore } from "@/features/core/ui/UIStore";
import { useWorksheetStore } from "@/features/core/worksheet/worksheetStore";
import { WorksheetService } from "@/services/worksheet/worksheetService";

import { showBlockingIpcError } from "./blockingErrorDialog";
import { resolveTabDisplayName } from "./resolveTabDisplayName";

export async function performWorksheetDelete(
  worksheetPath: string,
  context: ProjectCommandContext = captureProjectCommandContext(),
): Promise<boolean> {
  const document =
    useWorksheetStore.getState().documents[worksheetPath] ??
    (await WorksheetService.loadWorksheet(context.projectInstanceId, worksheetPath));
  if (!context.isCurrent()) return false;
  const committed = await WorksheetService.removeWorksheet(
    context.projectInstanceId,
    context.operationId,
    worksheetPath,
    document.revision,
  );
  if (!context.isCurrent()) return false;
  await projectPublicationCoordinator.submit({ result: committed });
  return context.isCurrent();
}

export async function deleteWorksheetWithConfirm(worksheetPath: string): Promise<boolean> {
  const name = resolveTabDisplayName({ id: worksheetPath, kind: "worksheet" }, worksheetPath);
  const context = captureProjectCommandContext();
  const confirmed = await uiStore.confirm({
    title: "删除工作表",
    message: `确定要删除工作表「${name}」吗？`,
    confirmText: "删除",
    cancelText: "取消",
    type: "danger",
  });
  if (!confirmed || !context.isCurrent()) return false;

  try {
    return await performWorksheetDelete(worksheetPath, context);
  } catch (error) {
    if (!context.isCurrent()) return false;
    showBlockingIpcError(error, "remove_worksheet", (code) =>
      i18n.t("notifications.editor.worksheetDeleteFailed", { error: code }),
    );
    return false;
  }
}
