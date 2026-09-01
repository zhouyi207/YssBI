import { useChartDocumentStore } from "@/features/core/chart/chartDocumentStore";
import { isResourceDocumentDirty } from "@/features/core/resource";
import { ChartService } from "@/services/chart/chartService";
import type { ResourceMutationResultDto } from "@/shared/types/domain/editorMutation";
import type { ChartDocument } from "@/shared/types/domain/chart";
import { captureProjectCommandContext } from "@/features/application/projectCommandContext";
import { projectPublicationCoordinator } from "@/features/application/editorMutation/projectPublicationCoordinator";

function savedDocumentFromResult(
  result: ResourceMutationResultDto,
  chartPath: string,
  operationId: string,
  before: ChartDocument,
): ChartDocument | null {
  const delta = result.deltas.find(
    (candidate) =>
      candidate.resource.kind === "chart" &&
      candidate.resource.key === chartPath &&
      candidate.causedBy === operationId &&
      candidate.fromRevision === before.revision &&
      candidate.payload.kind === "chart",
  );
  if (!delta || delta.payload.kind !== "chart") return null;
  return {
    ...before,
    ...delta.payload.patch.after,
    encodings: { ...delta.payload.patch.after.encodings },
    revision: delta.toRevision,
  };
}

function sameChartDocument(left: ChartDocument, right: ChartDocument): boolean {
  return (
    left.schemaVersion === right.schemaVersion &&
    left.revision === right.revision &&
    left.databaseId === right.databaseId &&
    left.chartType === right.chartType &&
    left.encodings.x === right.encodings.x &&
    left.encodings.y === right.encodings.y
  );
}

/** Saves the current chart draft through the Application mutation owner. */
export async function saveChartDocument(chartPath: string): Promise<boolean> {
  const document = useChartDocumentStore.getState().documents[chartPath];
  if (!document) return false;
  const context = captureProjectCommandContext();
  const result = await ChartService.saveChart(
    context.projectInstanceId,
    context.operationId,
    chartPath,
    document.revision,
    document,
  );
  if (!context.isCurrent()) return false;

  const expected = savedDocumentFromResult(result, chartPath, context.operationId, document);
  await projectPublicationCoordinator.submit({ result });
  if (!context.isCurrent() || !expected) return false;
  const settled = useChartDocumentStore.getState().documents[chartPath];
  return (
    settled !== undefined &&
    sameChartDocument(settled, expected) &&
    !isResourceDocumentDirty({ id: chartPath, kind: "chart" })
  );
}
