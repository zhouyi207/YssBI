import { beforeEach, expect, it, vi } from "vitest";
import { ChartService } from "@/services/chart/chartService";
import { resourceKey, useResourceStore } from "@/features/core/resource";
import { startProjectLifecycle } from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import type { ChartDocument } from "@/shared/types/domain/chart";
import { loadChartDocumentForView } from "./chartViewActions";

vi.mock("@/services/chart/chartService", () => ({ ChartService: { loadChart: vi.fn() } }));

const path = "charts/Report.yssbi-chart";
const ref = { id: path, kind: "chart" as const };
const document: ChartDocument = {
  schemaVersion: 4,
  databaseId: "sales",
  chartType: "scatter",
  encodings: { x: "time", y: "amount" },
};

function deferred() {
  let resolve!: (value: ChartDocument) => void;
  const promise = new Promise<ChartDocument>((complete) => {
    resolve = complete;
  });
  return { promise, resolve };
}

beforeEach(() => {
  vi.resetAllMocks();
  startProjectLifecycle("project-a");
  useResourceStore.getState().clear();
  useResourceStore.getState().setSnapshot({
    publicationRevision: 7,
    resources: [
      {
        ...ref,
        uri: resourceKey(ref),
        name: "Report",
        revision: 1,
        exists: true,
        loaded: false,
        hasDirtyDocument: false,
        hasStaleDocument: false,
        hasConflictDocument: false,
      },
    ],
  });
});

it("shares the editor and Details initial read and reuses an existing draft", async () => {
  const read = deferred();
  vi.mocked(ChartService.loadChart).mockReturnValueOnce(read.promise);
  const editor = loadChartDocumentForView(path);
  const details = loadChartDocumentForView(path);
  expect(ChartService.loadChart).toHaveBeenCalledOnce();
  expect(ChartService.loadChart).toHaveBeenCalledWith("project-a", path, 7);
  read.resolve(document);
  await expect(editor).resolves.toBe(document);
  await expect(details).resolves.toBe(document);

  const draft = useResourceStore.getState().updateChartDocument(path, { chartType: "line" });
  await expect(loadChartDocumentForView(path)).resolves.toBe(draft);
  expect(ChartService.loadChart).toHaveBeenCalledOnce();
});

it("keeps a document published and edited while an initial read was pending", async () => {
  const read = deferred();
  vi.mocked(ChartService.loadChart).mockReturnValueOnce(read.promise);
  const loading = loadChartDocumentForView(path);
  useResourceStore.getState().upsertChartDocument(path, document);
  const draft = useResourceStore.getState().updateChartDocument(path, { chartType: "line" });
  read.resolve(document);

  await expect(loading).resolves.toBe(draft);
  expect(useResourceStore.getState().chartDocuments[path]).toBe(draft);
});

it("invalidates a closing chart's read and permits a fresh read on reopening", async () => {
  const oldRead = deferred();
  const newRead = deferred();
  vi.mocked(ChartService.loadChart)
    .mockReturnValueOnce(oldRead.promise)
    .mockReturnValueOnce(newRead.promise);
  const closing = loadChartDocumentForView(path);
  useResourceStore.getState().removeChartDocument(path);
  const reopening = loadChartDocumentForView(path);
  expect(ChartService.loadChart).toHaveBeenCalledTimes(2);
  oldRead.resolve(document);
  await expect(closing).resolves.toBeNull();
  expect(useResourceStore.getState().chartDocuments).toEqual({});
  newRead.resolve(document);
  await expect(reopening).resolves.toBe(document);
});

it("does not install a previous project's response into the new project", async () => {
  const read = deferred();
  vi.mocked(ChartService.loadChart).mockReturnValueOnce(read.promise);
  const loading = loadChartDocumentForView(path);
  startProjectLifecycle("project-b");
  useResourceStore.getState().clear();
  read.resolve(document);

  await expect(loading).resolves.toBeNull();
  expect(useResourceStore.getState().chartDocuments).toEqual({});
});

it("rejects stale resources and does not share an older publication's pending read", async () => {
  const read = deferred();
  vi.mocked(ChartService.loadChart).mockReturnValueOnce(read.promise);
  const loading = loadChartDocumentForView(path);
  useResourceStore.getState().patchResource(ref, { revision: 2 });
  read.resolve(document);

  await expect(loading).resolves.toBeNull();
  expect(useResourceStore.getState().chartDocuments).toEqual({});

  const oldPublication = deferred();
  const newPublication = deferred();
  vi.mocked(ChartService.loadChart)
    .mockReturnValueOnce(oldPublication.promise)
    .mockReturnValueOnce(newPublication.promise);
  const oldLoading = loadChartDocumentForView(path);
  const current = useResourceStore.getState();
  current.setSnapshot({ resources: Object.values(current.resources), publicationRevision: 8 });
  const newLoading = loadChartDocumentForView(path);
  expect(ChartService.loadChart).toHaveBeenCalledTimes(3);
  expect(ChartService.loadChart).toHaveBeenLastCalledWith("project-a", path, 8);
  oldPublication.resolve(document);
  await expect(oldLoading).resolves.toBeNull();
  newPublication.resolve(document);
  await expect(newLoading).resolves.toBe(document);
});
