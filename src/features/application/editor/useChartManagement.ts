import { useCallback } from "react";
import { useTranslation } from "react-i18next";
import { DEFAULT_CHART_NAME } from "@/shared/constants/defaultResourceNames";
import { useChartDocumentStore } from "@/features/core/chart/chartDocumentStore";
import { ChartService } from "@/services/chart/chartService";
import { projectPublicationCoordinator } from "@/features/application/editorMutation/projectPublicationCoordinator";
import { captureProjectCommandContext } from "@/features/application/projectCommandContext";

import { revealWorkbenchView } from "@/modules/workbench/public";
import { PROJECT_TREE_CATEGORY_IDS, useSidebarStore } from "@/features/core/sidebar";
import { isEditorOpenRejectionHandled, openEditorPanel } from "./openEditorPanel";
import { showBlockingIpcError } from "./blockingErrorDialog";

function createdChartState(
  result: import("@/shared/types/domain/editorMutation").ResourceMutationResultDto,
  operationId: string,
) {
  const lifecycle = result.deltas.find(
    (delta) =>
      delta.resource.kind === "chart" &&
      delta.causedBy === operationId &&
      delta.payload.kind === "resource_lifecycle" &&
      delta.payload.patch.before === null &&
      delta.payload.patch.after?.kind === "chart",
  );
  return lifecycle?.payload.kind === "resource_lifecycle" ? lifecycle.payload.patch.after : null;
}

export function useChartManagement(openChart: (chartPath: string, name: string) => Promise<void>) {
  const { t } = useTranslation();

  const addChart = useCallback(
    async (databaseId?: string) => {
      let context: ReturnType<typeof captureProjectCommandContext> | undefined;
      try {
        context = captureProjectCommandContext();
        const created = await ChartService.createChart(
          context.projectInstanceId,
          context.operationId,
          DEFAULT_CHART_NAME,
          databaseId,
        );
        if (!context.isCurrent()) return;
        const createdState = createdChartState(created, context.operationId);
        if (!createdState) throw new Error("chart create result has no lifecycle insert");
        await projectPublicationCoordinator.submit({ result: created });
        if (!context.isCurrent()) return;

        await openChart(createdState.path, createdState.name);
        if (!context.isCurrent()) return;
      } catch (error) {
        if (context && !context.isCurrent()) return;
        if (isEditorOpenRejectionHandled(error)) return;
        showBlockingIpcError(error, "create_chart", (code) =>
          t("notifications.chart.createFailed", { error: code }),
        );
      }
    },
    [openChart, t],
  );

  const duplicateChart = useCallback(
    async (chartPath: string) => {
      let context: ReturnType<typeof captureProjectCommandContext> | undefined;
      try {
        context = captureProjectCommandContext();
        const indexEntry = useChartDocumentStore
          .getState()
          .index.find((chart) => chart.chartPath === chartPath);
        if (!indexEntry) throw new Error("chart has no authoritative index revision");
        const duplicated = await ChartService.duplicateChart(
          context.projectInstanceId,
          context.operationId,
          chartPath,
          indexEntry.revision,
        );
        if (!context.isCurrent()) return;
        const duplicatedState = createdChartState(duplicated, context.operationId);
        if (!duplicatedState) throw new Error("chart duplicate result has no lifecycle insert");
        await projectPublicationCoordinator.submit({ result: duplicated });
        if (!context.isCurrent()) return;
        await openChart(duplicatedState.path, duplicatedState.name);
      } catch (error) {
        if (context && !context.isCurrent()) return;
        if (isEditorOpenRejectionHandled(error)) return;
        showBlockingIpcError(error, "duplicate_chart", (code) =>
          t("notifications.chart.duplicateFailed", { error: code }),
        );
      }
    },
    [openChart, t],
  );

  return { addChart, duplicateChart };
}

export function useOpenChart() {
  return useCallback(async (chartPath: string, _name: string) => {
    if (!useChartDocumentStore.getState().documents[chartPath]) {
      const context = captureProjectCommandContext();
      try {
        const loaded = await ChartService.loadChart(context.projectInstanceId, chartPath);
        if (!context.isCurrent()) return;
        useChartDocumentStore.getState().upsertDocument(chartPath, loaded);
      } catch {
        if (!context.isCurrent()) return;
        // Index-only open: ChartEditor retries load on mount.
      }
    }

    try {
      await openEditorPanel(
        { resourceRef: chartPath, resourceKind: "chart" },
        {
          focusDetail: { kind: "chart", chartPath },
        },
      );
      void revealWorkbenchView("project");
      useSidebarStore
        .getState()
        .setCategoryExpanded("project", PROJECT_TREE_CATEGORY_IDS.charts, true);
    } catch (error) {
      if (!isEditorOpenRejectionHandled(error)) throw error;
    }
  }, []);
}
