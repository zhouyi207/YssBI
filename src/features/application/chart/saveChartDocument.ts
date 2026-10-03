import { useResourceStore } from "@/features/core/resource";
import { ChartService } from "@/services/chart/chartService";
import type { ChartDocument } from "@/shared/types/domain/chart";
import { captureProjectCommandContext } from "@/features/application/projectCommandContext";
import { projectPublicationCoordinator } from "@/features/application/editorMutation/projectPublicationCoordinator";

/** Saves the current chart draft through the Application mutation owner. */
export async function saveChartDocument(chartPath: string): Promise<boolean> {
  const document = useResourceStore.getState().chartDocuments[chartPath];
  if (!document) return false;
  const context = captureProjectCommandContext();
  const result = await ChartService.saveChart(
    context.projectInstanceId,
    context.operationId,
    chartPath,
    document,
  );
  if (!context.isCurrent()) return false;

  await projectPublicationCoordinator.submit({ result });
  if (!context.isCurrent()) return false;
  const delta = result.deltas.find(
    (candidate) =>
      candidate.resource.kind === "chart" &&
      candidate.resource.key === chartPath &&
      candidate.causedBy === context.operationId &&
      candidate.payload.kind === "chart",
  );
  if (!delta || delta.payload.kind !== "chart") return false;
  const expected: ChartDocument = {
    ...document,
    ...delta.payload.patch.after,
    encodings: { ...delta.payload.patch.after.encodings },
  };
  // Index publication preserves dirty drafts. Only this save's matching receipt can
  // acknowledge the submitted draft, even when an event or watcher installed the index first.
  return useResourceStore
    .getState()
    .settleChartSave(chartPath, document, expected, delta.toRevision);
}
