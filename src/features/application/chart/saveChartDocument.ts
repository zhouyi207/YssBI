import { useChartDocumentStore } from "@/features/core/chart/chartDocumentStore";
import {
  markResourceDirty,
  markResourceStale,
  resourceKey,
  useResourceStore,
} from "@/features/core/resource";
import { ChartService } from "@/services/chart/chartService";
import type { ChartDocument } from "@/shared/types/domain/chart";
import { captureProjectCommandContext } from "@/features/application/projectCommandContext";
import { projectPublicationCoordinator } from "@/features/application/editorMutation/projectPublicationCoordinator";

function sameChartDocument(left: ChartDocument, right: ChartDocument): boolean {
  return (
    left.schemaVersion === right.schemaVersion &&
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
  const ref = { id: chartPath, kind: "chart" } as const;
  const resource = useResourceStore.getState().resources[resourceKey(ref)];
  let settled = useChartDocumentStore.getState().documents[chartPath];
  if (!settled || !resource?.exists || resource.revision !== delta.toRevision) return false;

  // Index publication preserves dirty drafts. Only this save's matching receipt can
  // acknowledge the submitted draft, even when an event or watcher installed the index first.
  const hasNewerEdits = !sameChartDocument(settled, document);
  if (!hasNewerEdits) {
    settled = expected;
    useChartDocumentStore.getState().upsertDocument(chartPath, settled);
  }
  markResourceStale(ref, false);
  const saved = sameChartDocument(settled, expected);
  markResourceDirty(ref, !saved);
  return saved;
}
