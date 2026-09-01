import { beforeEach, describe, expect, it } from "vitest";

import {
  createChartPreviewCoordinator,
  type ChartPreviewIdentity,
} from "./chartPreviewCoordinator";
import type { ChartDocument, ChartPreviewPayload } from "@/shared/types/domain/chart";
import type { DeepReadonly } from "@/shared/types/deepReadonly";

const CHART_PATH = "charts/Report.yssbi-chart";

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((resolvePromise) => {
    resolve = resolvePromise;
  });
  return { promise, resolve };
}

function document(chartType: ChartDocument["chartType"]): ChartDocument {
  return {
    schemaVersion: 3,
    revision: 1,
    databaseId: "database-1",
    chartType,
    encodings: { x: "x", y: "y" },
  };
}

function preview(x: number): ChartPreviewPayload {
  return {
    kind: "scatter",
    pair: {
      data: [{ x, y: x }],
      xLabel: "x",
      yLabel: "y",
      xFormat: "number",
      yFormat: "number",
    },
  };
}

describe("Chart preview coordinator staged boundary", () => {
  let identity: ChartPreviewIdentity;

  beforeEach(() => {
    identity = { projectInstanceId: "project-a", epoch: 1 };
  });

  it("does not publish an in-flight preview after project replacement", async () => {
    const oldRequest = deferred<ChartPreviewPayload>();
    const published: DeepReadonly<ChartPreviewPayload>[] = [];
    const coordinator = createChartPreviewCoordinator({
      captureProjectIdentity: () => identity,
      service: {
        query: () => oldRequest.promise,
      },
      publication: {
        publish: (_request, value) => published.push(value),
        publishFailure: () => undefined,
      },
    });

    const completion = coordinator.query(CHART_PATH, document("scatter"));
    identity = { projectInstanceId: "project-b", epoch: 2 };
    coordinator.resetProject();
    oldRequest.resolve(preview(1));

    await expect(completion).resolves.toEqual({ status: "stale" });
    expect(published).toEqual([]);
    expect(coordinator.getCached(CHART_PATH, document("scatter"))).toBeUndefined();
  });

  it("suppresses an older document preview when a newer generation settles last", async () => {
    const oldRequest = deferred<ChartPreviewPayload>();
    const newRequest = deferred<ChartPreviewPayload>();
    const published: DeepReadonly<ChartPreviewPayload>[] = [];
    let calls = 0;
    const coordinator = createChartPreviewCoordinator({
      captureProjectIdentity: () => identity,
      service: {
        query: () => {
          calls += 1;
          return calls === 1 ? oldRequest.promise : newRequest.promise;
        },
      },
      publication: {
        publish: (_request, value) => published.push(value),
        publishFailure: () => undefined,
      },
    });

    const oldCompletion = coordinator.query(CHART_PATH, document("scatter"));
    const newCompletion = coordinator.query(CHART_PATH, document("line"));
    newRequest.resolve(preview(2));
    expect(await newCompletion).toEqual({ status: "published", value: preview(2) });
    oldRequest.resolve(preview(1));

    expect(await oldCompletion).toEqual({ status: "stale" });
    expect(published).toEqual([preview(2)]);
    expect(calls).toBe(2);
  });
});
