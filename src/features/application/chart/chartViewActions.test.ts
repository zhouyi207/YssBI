import { beforeEach, expect, it, vi } from "vitest";
import { ChartService } from "@/services/chart/chartService";
import { useChartDocumentStore } from "@/features/core/chart/chartDocumentStore";
import { resourceKey, useDocumentStateStore, useResourceStore } from "@/features/core/resource";
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
  useChartDocumentStore.getState().clear();
  useDocumentStateStore.getState().clear();
  useResourceStore.getState().clear();
  useResourceStore.getState().setSnapshot({
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
  read.resolve(document);
  await expect(editor).resolves.toBe(document);
  await expect(details).resolves.toBe(document);

  const draft = useChartDocumentStore.getState().updateDocument(path, { chartType: "line" });
  await expect(loadChartDocumentForView(path)).resolves.toBe(draft);
  expect(ChartService.loadChart).toHaveBeenCalledOnce();
});

it("keeps a document published and edited while an initial read was pending", async () => {
  const read = deferred();
  vi.mocked(ChartService.loadChart).mockReturnValueOnce(read.promise);
  const loading = loadChartDocumentForView(path);
  useChartDocumentStore.getState().upsertDocument(path, document);
  const draft = useChartDocumentStore.getState().updateDocument(path, { chartType: "line" });
  read.resolve(document);

  await expect(loading).resolves.toBe(draft);
  expect(useChartDocumentStore.getState().documents[path]).toBe(draft);
});

it("invalidates a closing chart's read and permits a fresh read on reopening", async () => {
  const oldRead = deferred();
  const newRead = deferred();
  vi.mocked(ChartService.loadChart)
    .mockReturnValueOnce(oldRead.promise)
    .mockReturnValueOnce(newRead.promise);
  const closing = loadChartDocumentForView(path);
  useChartDocumentStore.getState().removeDocument(path);
  const reopening = loadChartDocumentForView(path);
  expect(ChartService.loadChart).toHaveBeenCalledTimes(2);
  oldRead.resolve(document);
  await expect(closing).resolves.toBeNull();
  expect(useChartDocumentStore.getState().documents).toEqual({});
  newRead.resolve(document);
  await expect(reopening).resolves.toBe(document);
});

it("does not install a previous project's response into the new project", async () => {
  const read = deferred();
  vi.mocked(ChartService.loadChart).mockReturnValueOnce(read.promise);
  const loading = loadChartDocumentForView(path);
  startProjectLifecycle("project-b");
  useChartDocumentStore.getState().clear();
  read.resolve(document);

  await expect(loading).resolves.toBeNull();
  expect(useChartDocumentStore.getState().documents).toEqual({});
});

it("rejects a response from before a resource revision changed", async () => {
  const read = deferred();
  vi.mocked(ChartService.loadChart).mockReturnValueOnce(read.promise);
  const loading = loadChartDocumentForView(path);
  useResourceStore.getState().patchResource(ref, { revision: 2 });
  read.resolve(document);

  await expect(loading).resolves.toBeNull();
  expect(useChartDocumentStore.getState().documents).toEqual({});
});
