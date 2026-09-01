import { beforeEach, describe, expect, it, vi } from "vitest";
import { ChartService } from "@/services/chart/chartService";
import { useHistoryStore } from "@/features/core/history";
import { projectPublicationCoordinator } from "@/features/application/editorMutation/projectPublicationCoordinator";
import type { ChartDocument } from "@/shared/types/domain/chart";
import { useChartDocumentStore } from "./chartDocumentStore";
import { useProjectIOStore } from "@/features/application/project/projectIOStore";
import { saveChartDocument } from "@/features/application/chart/saveChartDocument";
import {
  isResourceDocumentDirty,
  markResourceDirty,
  resourceKey,
  useDocumentStateStore,
  useResourceStore,
} from "@/features/core/resource";

const projectInstanceId = "00000000-0000-0000-0000-000000000601";
const chartPath = "charts/Report.yssbi-chart";

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((settle) => {
    resolve = settle;
  });
  return { promise, resolve };
}

function chart(revision: number, chartType: ChartDocument["chartType"]): ChartDocument {
  return {
    schemaVersion: 3,
    revision,
    databaseId: "database-1",
    chartType,
    encodings: { x: "x", y: "y" },
  };
}

function registerChartResource(): void {
  useResourceStore.getState().upsertResource({
    id: chartPath,
    kind: "chart",
    name: "Report",
    uri: `yssbi://chart/${chartPath}`,
    exists: true,
    loaded: true,
    hasDirtyDocument: false,
    hasStaleDocument: false,
    hasConflictDocument: false,
  });
}

function chartResult(operationId: string, before: ChartDocument, after: ChartDocument) {
  return {
    operationId,
    projectInstanceId,
    publicationRevision: 1,
    moves: [],
    deltas: [
      {
        resource: { kind: "chart" as const, key: chartPath },
        fromRevision: before.revision,
        toRevision: after.revision,
        causedBy: operationId,
        payload: {
          kind: "chart" as const,
          patch: {
            before: {
              databaseId: before.databaseId,
              chartType: before.chartType,
              encodings: before.encodings,
            },
            after: {
              databaseId: after.databaseId,
              chartType: after.chartType,
              encodings: after.encodings,
            },
          },
        },
      },
    ],
    projectionReplacements: [],
    projectionStatus: { status: "complete" as const, expectedGraphPaths: [] },
    history: { canUndo: true, canRedo: false },
  };
}

describe("chart authoritative mutation results", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    useChartDocumentStore.getState().clear();
    useDocumentStateStore.getState().clear();
    useResourceStore.getState().clear();
    projectPublicationCoordinator.startProject(projectInstanceId, 0);
    useProjectIOStore.setState({ projectInstanceId });
    useHistoryStore.setState({ canUndo: false, canRedo: false, pending: false });
  });

  it("keys documents explicitly without synthesizing index rows", () => {
    const document = chart(3, "scatter");

    useChartDocumentStore.getState().upsertDocument(chartPath, document);

    expect(useChartDocumentStore.getState().documents).toEqual({ [chartPath]: document });
    expect(useChartDocumentStore.getState().index).toEqual([]);
  });

  it("ignores a delayed save completion from a replaced project", async () => {
    const draft = chart(3, "scatter");
    useChartDocumentStore.getState().upsertDocument(chartPath, draft);
    markResourceDirty({ id: chartPath, kind: "chart" }, true);
    const request = deferred<Awaited<ReturnType<typeof ChartService.saveChart>>>();
    vi.spyOn(ChartService, "saveChart").mockReturnValue(request.promise);

    const completion = saveChartDocument(chartPath);
    await vi.waitFor(() => expect(ChartService.saveChart).toHaveBeenCalled());
    useProjectIOStore.setState({ projectInstanceId: "project-b" });
    projectPublicationCoordinator.startProject("project-b", 0);
    useChartDocumentStore.getState().clear();
    request.resolve(chartResult("00000000-0000-0000-0000-000000000502", draft, chart(4, "line")));

    await expect(completion).resolves.toBe(false);
    expect(useChartDocumentStore.getState().documents).toEqual({});
    expect(projectPublicationCoordinator.getSnapshotForTests()).toMatchObject({
      projectInstanceId: "project-b",
      appliedRevision: 0,
    });
  });

  it("preserves a newer dirty edit while applying the save publication revision", async () => {
    const draft = chart(3, "scatter");
    const saved = chart(4, "scatter");
    registerChartResource();
    useChartDocumentStore.getState().upsertDocument(chartPath, draft);
    markResourceDirty({ id: chartPath, kind: "chart" }, true);
    const request = deferred<Awaited<ReturnType<typeof ChartService.saveChart>>>();
    vi.spyOn(ChartService, "saveChart").mockReturnValue(request.promise);

    const completion = saveChartDocument(chartPath);
    await vi.waitFor(() => expect(ChartService.saveChart).toHaveBeenCalled());
    useChartDocumentStore.getState().updateDocument(chartPath, { chartType: "line" });
    request.resolve(chartResult("00000000-0000-0000-0000-000000000503", draft, saved));

    await expect(completion).resolves.toBe(false);
    expect(useChartDocumentStore.getState().documents[chartPath]).toMatchObject({
      chartType: "line",
      revision: 4,
    });
    const key = resourceKey({ id: chartPath, kind: "chart" });
    expect(isResourceDocumentDirty({ id: chartPath, kind: "chart" })).toBe(true);
    expect(useDocumentStateStore.getState().documents[key]?.dirty).toBe(true);
    expect(useResourceStore.getState().resources[key]?.hasDirtyDocument).toBe(true);
    expect(projectPublicationCoordinator.getSnapshotForTests().appliedRevision).toBe(1);
  });

  it("clears dirty when an event-first save observes the submitted after state", async () => {
    const before = {
      ...chart(3, "histogram"),
      encodings: { x: "x", y: "standard-premium" },
    };
    const submitted = {
      ...before,
      encodings: { x: "x", y: "signed-premium" },
    };
    const authoritative = { ...submitted, revision: 4 };
    registerChartResource();
    useChartDocumentStore.getState().upsertDocument(chartPath, submitted);
    markResourceDirty({ id: chartPath, kind: "chart" }, true);
    const submit = vi.spyOn(projectPublicationCoordinator, "submit");
    vi.spyOn(ChartService, "saveChart").mockImplementation(
      async (_projectInstanceId, operationId) => {
        const result = chartResult(operationId, before, authoritative);
        void projectPublicationCoordinator.submit({ result });
        await vi.waitFor(() => {
          expect(projectPublicationCoordinator.getSnapshotForTests().appliedRevision).toBe(1);
        });
        expect(isResourceDocumentDirty({ id: chartPath, kind: "chart" })).toBe(false);
        return result;
      },
    );

    await expect(saveChartDocument(chartPath)).resolves.toBe(true);

    expect(useChartDocumentStore.getState().documents[chartPath]).toEqual(authoritative);
    expect(isResourceDocumentDirty({ id: chartPath, kind: "chart" })).toBe(false);
    expect(submit).toHaveBeenCalledTimes(2);
    expect(projectPublicationCoordinator.getSnapshotForTests().appliedRevision).toBe(1);
  });

  it("clears both dirty projections after a matching authoritative save", async () => {
    const draft = chart(3, "scatter");
    const authoritative = chart(4, "line");
    registerChartResource();
    useChartDocumentStore.getState().upsertDocument(chartPath, draft);
    markResourceDirty({ id: chartPath, kind: "chart" }, true);
    vi.spyOn(ChartService, "saveChart").mockImplementation(
      async (_projectInstanceId, operationId) => chartResult(operationId, draft, authoritative),
    );

    await expect(saveChartDocument(chartPath)).resolves.toBe(true);

    expect(ChartService.saveChart).toHaveBeenCalledWith(
      projectInstanceId,
      expect.any(String),
      chartPath,
      3,
      draft,
    );
    const key = resourceKey({ id: chartPath, kind: "chart" });
    expect(useChartDocumentStore.getState().documents[chartPath]).toEqual(authoritative);
    expect(useDocumentStateStore.getState().documents[key]?.dirty).toBe(false);
    expect(useResourceStore.getState().resources[key]?.hasDirtyDocument).toBe(false);
    expect(useHistoryStore.getState()).toMatchObject({ canUndo: true, canRedo: false });
  });
});
