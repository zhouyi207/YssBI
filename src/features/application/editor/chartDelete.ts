import i18n from "i18next";

import { projectPublicationCoordinator } from "@/features/application/editorMutation/projectPublicationCoordinator";
import {
  captureProjectCommandContext,
  type ProjectCommandContext,
} from "@/features/application/projectCommandContext";

import { uiStore } from "@/features/core/ui/UIStore";
import { useChartDocumentStore } from "@/features/core/chart/chartDocumentStore";
import { ChartService } from "@/services/chart/chartService";

import { showBlockingIpcError } from "./blockingErrorDialog";
import { resolveResourceDisplayName } from "./resolveResourceDisplayName";

export async function performChartDelete(
  chartPath: string,
  context: ProjectCommandContext = captureProjectCommandContext(),
): Promise<boolean> {
  const document =
    useChartDocumentStore.getState().documents[chartPath] ??
    (await ChartService.loadChart(context.projectInstanceId, chartPath));
  if (!context.isCurrent()) return false;
  const committed = await ChartService.removeChart(
    context.projectInstanceId,
    context.operationId,
    chartPath,
    document.revision,
  );
  if (!context.isCurrent()) return false;
  await projectPublicationCoordinator.submit({ result: committed });
  return context.isCurrent();
}

export async function deleteChartWithConfirm(chartPath: string): Promise<boolean> {
  const name = resolveResourceDisplayName({ id: chartPath, kind: "chart" }, chartPath);
  const context = captureProjectCommandContext();
  const confirmed = await uiStore.confirm({
    title: "删除图表",
    message: `确定要删除图表「${name}」吗？`,
    confirmText: "删除",
    cancelText: "取消",
    type: "danger",
  });
  if (!confirmed || !context.isCurrent()) return false;

  try {
    return await performChartDelete(chartPath, context);
  } catch (error) {
    if (!context.isCurrent()) return false;
    showBlockingIpcError(error, "remove_chart", (code) =>
      i18n.t("notifications.editor.chartDeleteFailed", { error: code }),
    );
    return false;
  }
}
