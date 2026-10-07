import { beforeEach, describe, expect, it, vi } from "vitest";
import { ChartService } from "@/services/chart/chartService";
import { ProjectService } from "@/services/project/projectService";
import { projectIndexSnapshotFixture } from "@/tests/helpers/activityPanelFixture";
import { projectPublicationCoordinator } from "@/features/application/editorMutation/projectPublicationCoordinator";
import type { ChartDocument } from "@/shared/types/domain/chart";
import { useProjectIOStore } from "@/features/application/project/projectIOStore";
import { saveChartDocument } from "@/features/application/chart/saveChartDocument";
import {
  isResourceDocumentDirty,
  markResourceDirty,
  resourceKey,
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
  useResourceStore.getState().setSnapshot({
    resources: [
      {
        id: chartPath,
        kind: "chart",
        name: "Report",
        uri: resourceKey({ id: chartPath, kind: "chart" }),
        exists: true,
        loaded: true,
        revision: committedRevision,
        hasDirtyDocument: false,
        hasStaleDocument: false,
        hasConflictDocument: false,
      },
    ],
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
        eventGraphs: [],
        functionGraphs: [],
        minds: [],
        docs: [],
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

  it("publishes chart content and resource flags together through load, edit, save and close", async () => {
    registerChartResource();
    const key = resourceKey({ id: chartPath, kind: "chart" });
    const observed: unknown[] = [];
    const stop = useResourceStore.subscribe((state) =>
      observed.push({
        chartType: useResourceStore.getState().chartDocuments[chartPath]?.chartType,
        loaded: state.documents[key]?.loaded,
        dirty: state.documents[key]?.dirty,
        summaryDirty: state.resources[key]?.hasDirtyDocument,
      }),
    );
    try {
      useResourceStore.getState().upsertChartDocument(chartPath, chart("scatter"));
      expect
        .soft(observed)
        .toEqual([{ chartType: "scatter", loaded: true, dirty: false, summaryDirty: false }]);
      observed.length = 0;
      const draft = useResourceStore
        .getState()
        .updateChartDocument(chartPath, { chartType: "line" })!;
      expect
        .soft(observed)
        .toEqual([{ chartType: "line", loaded: true, dirty: true, summaryDirty: true }]);
      observed.length = 0;
      const authoritative = chart("histogram");
      vi.spyOn(ChartService, "saveChart").mockImplementation(async (_project, operationId) =>
        commitChart(operationId, draft, authoritative),
      );
      expect(await saveChartDocument(chartPath)).toBe(true);
      expect
        .soft(
          observed.filter((value) => (value as { chartType: string }).chartType === "histogram"),
        )
        .toEqual([{ chartType: "histogram", loaded: true, dirty: false, summaryDirty: false }]);
      observed.length = 0;
      useResourceStore.getState().removeChartDocument(chartPath);
      expect(observed).toEqual([
        { chartType: undefined, loaded: undefined, dirty: undefined, summaryDirty: false },
      ]);
      observed.length = 0;
      useResourceStore.getState().setSnapshot({
        resources: Object.values(useResourceStore.getState().resources),
        documents: {},
        chartDocuments: { [chartPath]: chart("line") },
      });
      expect(observed).toEqual([
        { chartType: "line", loaded: true, dirty: false, summaryDirty: false },
      ]);
    } finally {
      stop();
    }
  });

  it("keeps chart references and clean state for equal installs and no-op edits", () => {
    registerChartResource();
    const document = chart("scatter");
    useResourceStore.getState().upsertChartDocument(chartPath, document);
    const before = useResourceStore.getState().chartDocuments;
    const notifications = vi.fn();
    const stop = useResourceStore.subscribe(notifications);
    try {
      useResourceStore.getState().upsertChartDocument(chartPath, structuredClone(document));
      const unchanged = useResourceStore.getState().updateChartDocument(chartPath, {
        chartType: "scatter",
        encodings: { x: "x" },
      });
      expect.soft(notifications).not.toHaveBeenCalled();
      expect.soft(unchanged).toBe(before[chartPath]);
      expect.soft(isResourceDocumentDirty({ id: chartPath, kind: "chart" })).toBe(false);
      notifications.mockClear();
      const changed = useResourceStore
        .getState()
        .updateChartDocument(chartPath, { chartType: "line" });
      expect(notifications).toHaveBeenCalledOnce();
      expect(changed?.encodings).toBe(before[chartPath].encodings);
      expect(before[chartPath].chartType).toBe("scatter");
    } finally {
      stop();
    }
  });

  it("ignores a delayed save completion from a replaced project", async () => {
    const draft = chart("scatter");
    useResourceStore.getState().upsertChartDocument(chartPath, draft);
    markResourceDirty({ id: chartPath, kind: "chart" }, true);
    const request = deferred<Awaited<ReturnType<typeof ChartService.saveChart>>>();
    vi.spyOn(ChartService, "saveChart").mockReturnValue(request.promise);

    const completion = saveChartDocument(chartPath);
    await vi.waitFor(() => expect(ChartService.saveChart).toHaveBeenCalled());
    useProjectIOStore.setState({ projectInstanceId: "project-b" });
    projectPublicationCoordinator.startProject("project-b", 0);
    useResourceStore.getState().clear();
    request.resolve(commitChart("00000000-0000-0000-0000-000000000502", draft, chart("line")));

    await expect(completion).resolves.toBe(false);
    expect(useResourceStore.getState().chartDocuments).toEqual({});
    expect(projectPublicationCoordinator.getSnapshotForTests()).toMatchObject({
      projectInstanceId: "project-b",
      appliedRevision: 0,
    });
  });

  it("preserves a newer dirty edit while applying the save publication revision", async () => {
    const draft = chart("scatter");
    const saved = chart("scatter");
    registerChartResource();
    useResourceStore.getState().upsertChartDocument(chartPath, draft);
    markResourceDirty({ id: chartPath, kind: "chart" }, true);
    const request = deferred<Awaited<ReturnType<typeof ChartService.saveChart>>>();
    const save = vi.spyOn(ChartService, "saveChart").mockReturnValue(request.promise);

    const completion = saveChartDocument(chartPath);
    await vi.waitFor(() => expect(ChartService.saveChart).toHaveBeenCalled());
    useResourceStore.getState().updateChartDocument(chartPath, { chartType: "line" });
    request.resolve(commitChart(save.mock.calls[0][1], draft, saved));

    await expect(completion).resolves.toBe(false);
    expect(useResourceStore.getState().chartDocuments[chartPath]).toMatchObject({
      chartType: "line",
    });
    const key = resourceKey({ id: chartPath, kind: "chart" });
    expect(isResourceDocumentDirty({ id: chartPath, kind: "chart" })).toBe(true);
    expect(useResourceStore.getState().documents[key]?.dirty).toBe(true);
    expect(useResourceStore.getState().resources[key]?.hasDirtyDocument).toBe(true);
    expect(useResourceStore.getState().resources[key]?.revision).toBe(4);
    expect(useResourceStore.getState().documents[key]?.conflict).toBe(false);
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
    useResourceStore.getState().upsertChartDocument(chartPath, submitted);
    markResourceDirty({ id: chartPath, kind: "chart" }, true);
    const submit = vi.spyOn(projectPublicationCoordinator, "submit");
    vi.spyOn(ChartService, "saveChart").mockImplementation(
      async (_projectInstanceId, operationId) => {
        const result = commitChart(operationId, before, authoritative);
        await projectPublicationCoordinator.submit({ result });
        expect(useResourceStore.getState().chartDocuments[chartPath]).toEqual(submitted);
        expect(isResourceDocumentDirty({ id: chartPath, kind: "chart" })).toBe(true);
        return result;
      },
    );

    await expect(saveChartDocument(chartPath)).resolves.toBe(true);

    expect(useResourceStore.getState().chartDocuments[chartPath]).toEqual(authoritative);
    expect(isResourceDocumentDirty({ id: chartPath, kind: "chart" })).toBe(false);
    expect(submit).toHaveBeenCalledTimes(2);
    expect(projectPublicationCoordinator.getSnapshotForTests().appliedRevision).toBe(1);
  });

  it("overwrites from an older draft and acknowledges subsequent saves", async () => {
    const draft = chart("scatter");
    const before = chart("histogram");
    const authoritative = chart("line");
    registerChartResource();
    useResourceStore.getState().upsertChartDocument(chartPath, draft);
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
    expect(useResourceStore.getState().chartDocuments[chartPath]).toEqual(authoritative);
    expect(useResourceStore.getState().documents[key]?.dirty).toBe(false);
    expect(useResourceStore.getState().resources[key]?.hasDirtyDocument).toBe(false);

    const nextDraft = useResourceStore.getState().updateChartDocument(chartPath, {
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
    expect(useResourceStore.getState().chartDocuments[chartPath]).toEqual(nextSaved);
    expect(isResourceDocumentDirty({ id: chartPath, kind: "chart" })).toBe(false);
  });

  it("acknowledges a save receipt already covered by a watcher snapshot", async () => {
    const draft = chart("scatter");
    const saved = draft;
    registerChartResource();
    useResourceStore.getState().upsertChartDocument(chartPath, draft);
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
    expect(useResourceStore.getState().chartDocuments[chartPath]).toEqual(saved);
    expect(isResourceDocumentDirty({ id: chartPath, kind: "chart" })).toBe(false);
  });

  it("preserves a conflicting draft when the snapshot includes a later external chart edit", async () => {
    const draft = chart("scatter");
    const saved = draft;
    registerChartResource();
    useResourceStore.getState().upsertChartDocument(chartPath, draft);
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
    expect(useResourceStore.getState().chartDocuments[chartPath]).toEqual(draft);
    const key = resourceKey({ id: chartPath, kind: "chart" });
    expect(useResourceStore.getState().documents[key]).toMatchObject({
      dirty: true,
      conflict: true,
    });
    expect(useResourceStore.getState().resources[key]).toMatchObject({
      revision: 5,
      hasConflictDocument: true,
    });
  });
});
