import { ChartService } from "@/services/chart/chartService";
import { useChartDocumentStore } from "@/features/core/chart/chartDocumentStore";
import { captureProjectCommandContext } from "@/features/application/projectCommandContext";
import type { ChartDocument } from "@/shared/types/domain/chart";

/** Load a chart for a View and commit only if its captured project is current. */
export async function loadChartDocumentForView(chartPath: string): Promise<ChartDocument | null> {
  const context = captureProjectCommandContext();
  try {
    const document = await ChartService.loadChart(context.projectInstanceId, chartPath);
    if (!context.isCurrent()) return null;
    useChartDocumentStore.getState().upsertDocument(chartPath, document);
    return document;
  } catch {
    return null;
  }
}
