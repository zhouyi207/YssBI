import { beforeEach, describe, expect, it, vi } from "vitest";
import { ChartService } from "@/services/chart/chartService";
import { ProjectService } from "@/services/project/projectService";
import { projectIndexSnapshotFixture } from "@/tests/helpers/activityPanelFixture";
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
let committedDocument: ChartDocument;
let committedRevision: number;
let publicationRevision: number;

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((settle) => {
    resolve = settle;
  });
  return { promise, resolve };
}

function chart(chartType: ChartDocument["chartType"]): ChartDocument {
  return {
    schemaVersion: 4,
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
    revision: committedRevision,
    hasDirtyDocument: false,
    hasStaleDocument: false,
    hasConflictDocument: false,
  });
}

function commitChart(operationId: string, before: ChartDocument, after: ChartDocument) {
  const fromRevision = committedRevision;
  committedRevision += 1;
  committedDocument = after;
  publicationRevision += 1;
  return {
    operationId,
    projectInstanceId,
    publicationRevision,
    moves: [],
    deltas: [
      {
        resource: { kind: "chart" as const, key: chartPath },
        fromRevision,
        toRevision: committedRevision,
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
    committedDocument = chart("scatter");
    committedRevision = 3;
    publicationRevision = 0;
    vi.spyOn(ProjectService, "getProjectIndex").mockImplementation(async () =>
      projectIndexSnapshotFixture({
        projectInstanceId,
        projectName: "Project",
        exportTime: "",
        publicationRevision,
        graphs: [],
        charts: [
          {
            chartPath,
            name: "Report",
            databaseId: committedDocument.databaseId,
            chartType: committedDocument.chartType,
            revision: committedRevision,
          },
        ],
        databases: [],
      }),
    );
    vi.spyOn(ChartService, "loadChart").mockImplementation(async () => committedDocument);
  });

  it("keys documents explicitly without synthesizing index rows", () => {
    const document = chart("scatter");

    useChartDocumentStore.getState().upsertDocument(chartPath, document);

    expect(useChartDocumentStore.getState().documents).toEqual({ [chartPath]: document });
    expect(useChartDocumentStore.getState().index).toEqual([]);
  });

  it("ignores a delayed save completion from a replaced project", async () => {
    const draft = chart("scatter");
    useChartDocumentStore.getState().upsertDocument(chartPath, draft);
    markResourceDirty({ id: chartPath, kind: "chart" }, true);
    const request = deferred<Awaited<ReturnType<typeof ChartService.saveChart>>>();
    vi.spyOn(ChartService, "saveChart").mockReturnValue(request.promise);

    const completion = saveChartDocument(chartPath);
    await vi.waitFor(() => expect(ChartService.saveChart).toHaveBeenCalled());
    useProjectIOStore.setState({ projectInstanceId: "project-b" });
    projectPublicationCoordinator.startProject("project-b", 0);
    useChartDocumentStore.getState().clear();
    request.resolve(commitChart("00000000-0000-0000-0000-000000000502", draft, chart("line")));

    await expect(completion).resolves.toBe(false);
    expect(useChartDocumentStore.getState().documents).toEqual({});
    expect(projectPublicationCoordinator.getSnapshotForTests()).toMatchObject({
      projectInstanceId: "project-b",
      appliedRevision: 0,
    });
  });

  it("preserves a newer dirty edit while applying the save publication revision", async () => {
    const draft = chart("scatter");
    const saved = chart("scatter");
    registerChartResource();
    useChartDocumentStore.getState().upsertDocument(chartPath, draft);
    markResourceDirty({ id: chartPath, kind: "chart" }, true);
    const request = deferred<Awaited<ReturnType<typeof ChartService.saveChart>>>();
    const save = vi.spyOn(ChartService, "saveChart").mockReturnValue(request.promise);

    const completion = saveChartDocument(chartPath);
    await vi.waitFor(() => expect(ChartService.saveChart).toHaveBeenCalled());
    useChartDocumentStore.getState().updateDocument(chartPath, { chartType: "line" });
    request.resolve(commitChart(save.mock.calls[0][1], draft, saved));

    await expect(completion).resolves.toBe(false);
    expect(useChartDocumentStore.getState().documents[chartPath]).toMatchObject({
      chartType: "line",
    });
    const key = resourceKey({ id: chartPath, kind: "chart" });
    expect(isResourceDocumentDirty({ id: chartPath, kind: "chart" })).toBe(true);
    expect(useDocumentStateStore.getState().documents[key]?.dirty).toBe(true);
    expect(useResourceStore.getState().resources[key]?.hasDirtyDocument).toBe(true);
    expect(useResourceStore.getState().resources[key]?.revision).toBe(4);
    expect(useDocumentStateStore.getState().documents[key]?.conflict).toBe(false);
    expect(useResourceStore.getState().resources[key]?.hasConflictDocument).toBe(false);
    expect(projectPublicationCoordinator.getSnapshotForTests().appliedRevision).toBe(1);
  });

  it("acknowledges the saved draft after an event-first publication", async () => {
    const before = {
      ...chart("histogram"),
      encodings: { x: "x", y: "standard-premium" },
    };
    const submitted = {
      ...before,
      encodings: { x: "x", y: "signed-premium" },
    };
    const authoritative = submitted;
    registerChartResource();
    useChartDocumentStore.getState().upsertDocument(chartPath, submitted);
    markResourceDirty({ id: chartPath, kind: "chart" }, true);
    const submit = vi.spyOn(projectPublicationCoordinator, "submit");
    vi.spyOn(ChartService, "saveChart").mockImplementation(
      async (_projectInstanceId, operationId) => {
        const result = commitChart(operationId, before, authoritative);
        await projectPublicationCoordinator.submit({ result });
        expect(useChartDocumentStore.getState().documents[chartPath]).toEqual(submitted);
        expect(isResourceDocumentDirty({ id: chartPath, kind: "chart" })).toBe(true);
        return result;
      },
    );

    await expect(saveChartDocument(chartPath)).resolves.toBe(true);

    expect(useChartDocumentStore.getState().documents[chartPath]).toEqual(authoritative);
    expect(isResourceDocumentDirty({ id: chartPath, kind: "chart" })).toBe(false);
    expect(submit).toHaveBeenCalledTimes(2);
    expect(projectPublicationCoordinator.getSnapshotForTests().appliedRevision).toBe(1);
  });

  it("overwrites from an older draft and acknowledges subsequent saves", async () => {
    const draft = chart("scatter");
    const before = chart("histogram");
    const authoritative = chart("line");
    registerChartResource();
    useChartDocumentStore.getState().upsertDocument(chartPath, draft);
    markResourceDirty({ id: chartPath, kind: "chart" }, true);
    committedDocument = before;
    committedRevision = 4;
    const save = vi
      .spyOn(ChartService, "saveChart")
      .mockImplementationOnce(async (_projectInstanceId, operationId) =>
        commitChart(operationId, before, authoritative),
      );

    await expect(saveChartDocument(chartPath)).resolves.toBe(true);

    expect(ChartService.saveChart).toHaveBeenCalledWith(
      projectInstanceId,
      expect.any(String),
      chartPath,
      draft,
    );
    const key = resourceKey({ id: chartPath, kind: "chart" });
    expect(useChartDocumentStore.getState().documents[chartPath]).toEqual(authoritative);
    expect(useDocumentStateStore.getState().documents[key]?.dirty).toBe(false);
    expect(useResourceStore.getState().resources[key]?.hasDirtyDocument).toBe(false);

    const nextDraft = useChartDocumentStore.getState().updateDocument(chartPath, {
      encodings: { y: "next-y" },
    })!;
    const nextSaved = nextDraft;
    save.mockImplementationOnce(async (_projectInstanceId, operationId) =>
      commitChart(operationId, authoritative, nextSaved),
    );

    await expect(saveChartDocument(chartPath)).resolves.toBe(true);
    expect(save).toHaveBeenLastCalledWith(
      projectInstanceId,
      expect.any(String),
      chartPath,
      nextDraft,
    );
    expect(useChartDocumentStore.getState().documents[chartPath]).toEqual(nextSaved);
    expect(isResourceDocumentDirty({ id: chartPath, kind: "chart" })).toBe(false);
  });

  it("acknowledges a save receipt already covered by a watcher snapshot", async () => {
    const draft = chart("scatter");
    const saved = draft;
    registerChartResource();
    useChartDocumentStore.getState().upsertDocument(chartPath, draft);
    markResourceDirty({ id: chartPath, kind: "chart" }, true);
    vi.spyOn(ChartService, "saveChart").mockImplementation(
      async (_projectInstanceId, operationId) => {
        const result = commitChart(operationId, draft, saved);
        await projectPublicationCoordinator.refreshIndex();
        expect(projectPublicationCoordinator.capturePublicationRevision()).toBe(1);
        return result;
      },
    );

    await expect(saveChartDocument(chartPath)).resolves.toBe(true);
    expect(useChartDocumentStore.getState().documents[chartPath]).toEqual(saved);
    expect(isResourceDocumentDirty({ id: chartPath, kind: "chart" })).toBe(false);
  });

  it("preserves a conflicting draft when the snapshot includes a later external chart edit", async () => {
    const draft = chart("scatter");
    const saved = draft;
    registerChartResource();
    useChartDocumentStore.getState().upsertDocument(chartPath, draft);
    markResourceDirty({ id: chartPath, kind: "chart" }, true);
    vi.spyOn(ChartService, "saveChart").mockImplementation(
      async (_projectInstanceId, operationId) => {
        const result = commitChart(operationId, draft, saved);
        committedDocument = chart("histogram");
        committedRevision = 5;
        publicationRevision = 2;
        return result;
      },
    );

    await expect(saveChartDocument(chartPath)).resolves.toBe(false);
    expect(useChartDocumentStore.getState().documents[chartPath]).toEqual(draft);
    const key = resourceKey({ id: chartPath, kind: "chart" });
    expect(useDocumentStateStore.getState().documents[key]).toMatchObject({
      dirty: true,
      conflict: true,
    });
    expect(useResourceStore.getState().resources[key]).toMatchObject({
      revision: 5,
      hasConflictDocument: true,
    });
  });
});
