import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  createChartDocumentCoordinator,
  type ChartProjectIdentity,
} from "./chartDocumentCoordinator";
import { getChartSnapshot, type ChartCommittedSnapshot } from "@/features/core/chart/read";
import { chartProjectionPublication } from "@/features/core/chart/publication";
import { chartUi } from "@/features/core/chart/ui";
import type { ChartDocument, ChartIndexEntry } from "@/shared/types/domain/chart";

const CHART_PATH = "charts/Report.yssbi-chart";
const PROJECT_A = "project-a";
const PROJECT_B = "project-b";

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

function document(revision: number, chartType: ChartDocument["chartType"]): ChartDocument {
  return {
    schemaVersion: 3,
    revision,
    databaseId: "database-1",
    chartType,
    encodings: { x: "x", y: "y" },
  };
}

function indexEntry(chartPath: string, value: ChartDocument): ChartIndexEntry {
  return {
    chartPath,
    name: "Report",
    databaseId: value.databaseId,
    chartType: value.chartType,
    revision: value.revision,
  };
}

function pendingFor(path: string) {
  const records = getChartSnapshot().pendingSaveByPath[path];
  const record = records ? Object.values(records)[0] : undefined;
  expect(record).toBeDefined();
  return record!;
}

describe("Chart coordinator staged boundary", () => {
  beforeEach(() => {
    chartProjectionPublication.clearForProject(null);
    vi.restoreAllMocks();
  });

  it("publishes only the current project when an older load resolves after replacement", async () => {
    let identity: ChartProjectIdentity = {
      projectInstanceId: PROJECT_A,
      epoch: 1,
    };
    const loadA = deferred<ChartDocument>();
    const loadB = deferred<ChartDocument>();
    const publishIssue = vi.fn();
    const service = {
      loadChart: vi.fn((projectInstanceId: string) =>
        projectInstanceId === PROJECT_A ? loadA.promise : loadB.promise,
      ),
      saveChart: vi.fn().mockResolvedValue({ accepted: true }),
    };
    const coordinator = createChartDocumentCoordinator({
      captureProjectIdentity: () => identity,
      service,
      publishIssue,
    });

    const completionA = coordinator.load(CHART_PATH);
    identity = { projectInstanceId: PROJECT_B, epoch: 2 };
    coordinator.resetProject();
    const completionB = coordinator.load(CHART_PATH);

    loadB.resolve(document(8, "line"));
    expect(await completionB).toEqual({ status: "loaded" });

    loadA.resolve(document(7, "scatter"));
    expect(await completionA).toEqual({ status: "stale" });
    expect(getChartSnapshot()).toMatchObject({
      documents: {
        [CHART_PATH]: document(8, "line"),
      },
      draftsByPath: {},
      dirtyByPath: {},
      pendingSaveByPath: {},
    });
    expect(publishIssue).not.toHaveBeenCalled();
    expect(service.loadChart).toHaveBeenCalledTimes(2);
  });

  it("keeps ordinary acknowledgements separate from matching committed rebases", async () => {
    const identity: ChartProjectIdentity = {
      projectInstanceId: PROJECT_A,
      epoch: 1,
    };
    const base = document(3, "scatter");
    const saved = document(4, "line");
    const service = {
      loadChart: vi.fn().mockResolvedValue(base),
      saveChart: vi.fn().mockResolvedValue({ accepted: true }),
    };
    const coordinator = createChartDocumentCoordinator({
      captureProjectIdentity: () => identity,
      service,
    });
    coordinator.resetProject();
    chartProjectionPublication.replaceSnapshot({
      index: [indexEntry(CHART_PATH, base)],
      documents: { [CHART_PATH]: base },
    } satisfies ChartCommittedSnapshot);
    chartUi.updateDraft(CHART_PATH, { chartType: "line" });

    await expect(coordinator.save(CHART_PATH)).resolves.toEqual({
      status: "acknowledged",
    });
    const acknowledged = pendingFor(CHART_PATH);
    expect(acknowledged.status).toBe("acknowledged");
    expect(getChartSnapshot().draftsByPath[CHART_PATH]).toMatchObject({
      chartType: "line",
    });
    expect(getChartSnapshot().dirtyByPath[CHART_PATH]).toBe(true);

    const committed = {
      ...saved,
      encodings: { x: "x", y: "y" },
    };
    expect(coordinator.acceptCommittedDocument(CHART_PATH, committed, acknowledged)).toBe(
      "rebased",
    );
    expect(getChartSnapshot().documents[CHART_PATH]).toEqual(committed);
    expect(getChartSnapshot().draftsByPath[CHART_PATH]).toBeUndefined();
    expect(getChartSnapshot().dirtyByPath[CHART_PATH]).toBe(false);
    expect(getChartSnapshot().pendingSaveByPath[CHART_PATH]).toBeUndefined();

    chartUi.updateDraft(CHART_PATH, { chartType: "scatter" });
    await expect(coordinator.save(CHART_PATH)).resolves.toEqual({
      status: "acknowledged",
    });
    const secondSave = pendingFor(CHART_PATH);
    chartUi.updateDraft(CHART_PATH, { chartType: "histogram" });

    expect(
      coordinator.acceptCommittedDocument(CHART_PATH, document(5, "scatter"), secondSave),
    ).toBe("draft-changed");
    expect(getChartSnapshot().documents[CHART_PATH]?.chartType).toBe("scatter");
    expect(getChartSnapshot().draftsByPath[CHART_PATH]?.chartType).toBe("histogram");
    expect(getChartSnapshot().dirtyByPath[CHART_PATH]).toBe(true);
    expect(getChartSnapshot().pendingSaveByPath[CHART_PATH]).toBeUndefined();
  });
});
