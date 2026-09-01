import { useCallback } from "react";
import { useTranslation } from "react-i18next";
import { DEFAULT_CHART_NAME } from "@/shared/constants/defaultResourceNames";
import { useChartDocumentStore } from "@/features/core/chart/chartDocumentStore";
import { commitFileFirstResourceIndex } from "@/features/application/resource/resourceActions";
import { ChartService } from "@/services/chart/chartService";
import { projectPublicationCoordinator } from "@/features/application/editorMutation/projectPublicationCoordinator";
import { captureProjectCommandContext } from "@/features/application/projectCommandContext";

import { revealWorkbenchView } from "@/modules/workbench/public";
import { PROJECT_TREE_CATEGORY_IDS, useSidebarStore } from "@/features/core/sidebar";
import { isEditorOpenRejectionHandled, openEditorPanel } from "./openEditorPanel";
import type { ChartDocument } from "@/shared/types/domain/chart";
import { showBlockingIpcError } from "./blockingErrorDialog";

interface StagedChartDocument {
  chartPath: string;
  staged: ChartDocument;
  previous: ChartDocument | undefined;
}

function stageChartDocument(chartPath: string, staged: ChartDocument): StagedChartDocument {
  const previous = useChartDocumentStore.getState().documents[chartPath];
  useChartDocumentStore.setState((state) => ({
    documents: { ...state.documents, [chartPath]: staged },
  }));
  return { chartPath, staged, previous };
}

function rollbackStagedChartDocument(stage: StagedChartDocument | undefined): void {
  if (!stage) return;
  useChartDocumentStore.setState((state) => {
    if (state.documents[stage.chartPath] !== stage.staged) return {};
    const documents = { ...state.documents };
    if (stage.previous) documents[stage.chartPath] = stage.previous;
    else delete documents[stage.chartPath];
    return { documents };
  });
}

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
      let stagedDocument: StagedChartDocument | undefined;
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
        const createdDocument = await ChartService.loadChart(
          context.projectInstanceId,
          createdState.path,
        );
        if (!context.isCurrent()) return;
        stagedDocument = stageChartDocument(createdState.path, createdDocument);
        await projectPublicationCoordinator.submit({ result: created });
        stagedDocument = undefined;
        if (!context.isCurrent()) return;

        await commitFileFirstResourceIndex();
        if (!context.isCurrent()) return;
        await openChart(createdState.path, createdState.name);
        if (!context.isCurrent()) return;
      } catch (error) {
        if (context && !context.isCurrent()) return;
        rollbackStagedChartDocument(stagedDocument);
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
      let stagedDocument: StagedChartDocument | undefined;
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
        const duplicatedDocument = await ChartService.loadChart(
          context.projectInstanceId,
          duplicatedState.path,
        );
        if (!context.isCurrent()) return;
        stagedDocument = stageChartDocument(duplicatedState.path, duplicatedDocument);
        await projectPublicationCoordinator.submit({ result: duplicated });
        stagedDocument = undefined;
        if (!context.isCurrent()) return;
        await commitFileFirstResourceIndex();
        if (!context.isCurrent()) return;
        await openChart(duplicatedState.path, duplicatedState.name);
      } catch (error) {
        if (context && !context.isCurrent()) return;
        rollbackStagedChartDocument(stagedDocument);
        if (isEditorOpenRejectionHandled(error)) return;
        showBlockingIpcError(error, "duplicate_chart", (code) =>
          t("notifications.chart.duplicateFailed", { error: code }),
        );
      }
    },
    [openChart, t],
  );

  const ensureChartLoaded = useCallback(async (chartPath: string) => {
    const cached = useChartDocumentStore.getState().documents[chartPath];
    if (cached) return cached;
    const context = captureProjectCommandContext();
    const document = await ChartService.loadChart(context.projectInstanceId, chartPath);
    if (!context.isCurrent()) return null;
    useChartDocumentStore.getState().upsertDocument(chartPath, document);
    return document;
  }, []);

  return { addChart, duplicateChart, ensureChartLoaded };
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
        { resourceRef: chartPath, resourceKind: "chart", pinned: true },
        {
          focusDetail: { kind: "chart", chartPath },
        },
      );
      void revealWorkbenchView("project");
      useSidebarStore
        .getState()
        .setProjectTreeCategoryExpanded(PROJECT_TREE_CATEGORY_IDS.charts, true);
    } catch (error) {
      if (!isEditorOpenRejectionHandled(error)) throw error;
    }
  }, []);
}
