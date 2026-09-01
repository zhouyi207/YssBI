import { beforeEach, describe, expect, it, vi } from "vitest";
import { projectPublicationCoordinator } from "@/features/application/editorMutation/projectPublicationCoordinator";
import {
  assertCurrentProjectIdentity,
  captureProjectIdentity,
  isCurrentProjectIdentity,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { DatabaseService } from "@/services/database/databaseService";
import { WorksheetService } from "@/services/worksheet/worksheetService";
import type {
  ColumnDistribution,
  PlotColumnPairPayload,
  WorksheetChartType,
  WorksheetDocument,
} from "@/shared/types/domain";
import { fetchWorksheetPreview } from "./worksheetDataService";

const projectInstanceId = "00000000-0000-0000-0000-000000000601";
const replacementProjectInstanceId = "00000000-0000-0000-0000-000000000602";

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((settle, fail) => {
    resolve = settle;
    reject = fail;
  });
  return { promise, resolve, reject };
}

function document(chartType: WorksheetChartType): WorksheetDocument {
  return {
    schemaVersion: 3,
    revision: 0,
    databaseId: "sales",
    chartType,
    encodings: chartType === "histogram" ? { x: "amount" } : { x: "amount", y: "cost" },
  };
}

function identity() {
  const snapshot = captureProjectIdentity();
  return {
    projectInstanceId: snapshot.projectInstanceId,
    isCurrent: () => isCurrentProjectIdentity(snapshot),
    assertCurrent: () => assertCurrentProjectIdentity(snapshot),
  };
}

const distribution: ColumnDistribution[] = [{ columnName: "amount", kind: "numeric", bins: [] }];
const pair: PlotColumnPairPayload = {
  data: [{ x: 1, y: 2 }],
  xLabel: "amount",
  yLabel: "cost",
  xFormat: "number",
  yFormat: "number",
};

describe("fetchWorksheetPreview project lifecycle ownership", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    projectPublicationCoordinator.cancelProject();
    projectPublicationCoordinator.startProject(projectInstanceId, 0);
  });

  it.each(["histogram", "scatter", "line"] as const)(
    "rejects a delayed %s resolve after project replacement",
    async (chartType) => {
      const request = deferred<ColumnDistribution[] | PlotColumnPairPayload>();
      if (chartType === "histogram") {
        vi.spyOn(DatabaseService, "getColumnDistribution").mockReturnValue(
          request.promise as Promise<ColumnDistribution[]>,
        );
      } else {
        vi.spyOn(WorksheetService, "getPlotColumnPair").mockReturnValue(
          request.promise as Promise<PlotColumnPairPayload>,
        );
      }

      const completion = fetchWorksheetPreview(document(chartType), identity());
      if (chartType === "histogram") {
        expect(DatabaseService.getColumnDistribution).toHaveBeenCalledWith(
          projectInstanceId,
          "sales",
        );
      } else {
        expect(WorksheetService.getPlotColumnPair).toHaveBeenCalledWith(
          projectInstanceId,
          "sales",
          "amount",
          "cost",
        );
      }
      projectPublicationCoordinator.startProject(replacementProjectInstanceId, 0);
      request.resolve(chartType === "histogram" ? distribution : pair);

      await expect(completion).rejects.toMatchObject({ code: "stale_project_lifecycle" });
    },
  );

  it.each(["histogram", "scatter", "line"] as const)(
    "rejects a delayed %s rejection after project replacement without exposing its error",
    async (chartType) => {
      const request = deferred<ColumnDistribution[] | PlotColumnPairPayload>();
      if (chartType === "histogram") {
        vi.spyOn(DatabaseService, "getColumnDistribution").mockReturnValue(
          request.promise as Promise<ColumnDistribution[]>,
        );
      } else {
        vi.spyOn(WorksheetService, "getPlotColumnPair").mockReturnValue(
          request.promise as Promise<PlotColumnPairPayload>,
        );
      }

      const completion = fetchWorksheetPreview(document(chartType), identity());
      projectPublicationCoordinator.startProject(replacementProjectInstanceId, 0);
      request.reject(new Error("old project failed"));

      await expect(completion).rejects.toMatchObject({ code: "stale_project_lifecycle" });
    },
  );
});
