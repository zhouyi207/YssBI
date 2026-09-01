import { describe, expect, it } from "vitest";
import type { ChartDocument, ChartPreviewPayload } from "@/shared/types/domain";
import {
  clearChartPreviewCache,
  getChartPreview as getChartPreviewForPath,
  getCachedChartPreview as getCachedChartPreviewForPath,
  getChartPreviewCacheSnapshotForTests,
  invalidateChartPreviewCacheForDatabase,
  invalidateChartPreviewCacheForMove,
  chartPreviewCacheKey as chartPreviewCacheKeyForPath,
} from "./chartPreviewCache";

const PROJECT_A = "00000000-0000-0000-0000-000000000601";
const PROJECT_B = "00000000-0000-0000-0000-000000000602";
const CHART_PATH = "charts/Chart.yssbi-chart";

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((settle, fail) => {
    resolve = settle;
    reject = fail;
  });
  return { promise, resolve, reject };
}

const DOCUMENT: ChartDocument = {
  schemaVersion: 3,
  revision: 0,
  databaseId: "database-1",
  chartType: "scatter",
  encodings: { x: "x", y: "y" },
};

function chartPreviewCacheKey(projectInstanceId: string, document: ChartDocument): string {
  return chartPreviewCacheKeyForPath(projectInstanceId, CHART_PATH, document);
}

function getCachedChartPreview(
  projectInstanceId: string,
  document: ChartDocument,
): ChartPreviewPayload | undefined {
  return getCachedChartPreviewForPath(projectInstanceId, CHART_PATH, document);
}

function getChartPreview(
  projectInstanceId: string,
  document: ChartDocument,
  loader: () => Promise<ChartPreviewPayload>,
): Promise<ChartPreviewPayload> {
  return getChartPreviewForPath(projectInstanceId, CHART_PATH, document, loader);
}

describe("chartPreviewCacheKey", () => {
  it("is stable for equivalent encoding objects", () => {
    const left = chartPreviewCacheKey(PROJECT_A, {
      ...DOCUMENT,
      encodings: { x: "x", y: "y" },
    });
    const right = chartPreviewCacheKey(PROJECT_A, {
      ...DOCUMENT,
      encodings: { y: "y", x: "x" },
    });

    expect(left).toBe(right);
  });

  it("separates the same chart and database IDs by project identity", () => {
    expect(chartPreviewCacheKey(PROJECT_A, DOCUMENT)).not.toBe(
      chartPreviewCacheKey(PROJECT_B, DOCUMENT),
    );
  });
});

describe("getChartPreview", () => {
  it("reuses completed previews for the same chart spec", async () => {
    clearChartPreviewCache();
    let calls = 0;
    const preview: ChartPreviewPayload = { kind: "empty" };

    const first = await getChartPreview(PROJECT_A, DOCUMENT, async () => {
      calls += 1;
      return preview;
    });
    const second = await getChartPreview(PROJECT_A, DOCUMENT, async () => {
      calls += 1;
      return { kind: "error", code: "should_not_run", incidentId: null };
    });

    expect(first).toBe(preview);
    expect(second).toBe(preview);
    expect(calls).toBe(1);
  });

  it("exposes a synchronous cached preview for immediate remounts", async () => {
    clearChartPreviewCache();
    const preview: ChartPreviewPayload = { kind: "empty" };

    expect(getCachedChartPreview(PROJECT_A, DOCUMENT)).toBeUndefined();
    await getChartPreview(PROJECT_A, DOCUMENT, async () => preview);

    expect(getCachedChartPreview(PROJECT_A, DOCUMENT)).toBe(preview);
  });

  it("deduplicates concurrent preview requests for the same chart spec", async () => {
    clearChartPreviewCache();
    let calls = 0;
    const preview: ChartPreviewPayload = { kind: "empty" };

    const [first, second] = await Promise.all([
      getChartPreview(PROJECT_A, DOCUMENT, async () => {
        calls += 1;
        return preview;
      }),
      getChartPreview(PROJECT_A, DOCUMENT, async () => {
        calls += 1;
        return { kind: "error", code: "should_not_run", incidentId: null };
      }),
    ]);

    expect(first).toBe(preview);
    expect(second).toBe(preview);
    expect(calls).toBe(1);
  });

  it("invalidates cached previews by database id", async () => {
    clearChartPreviewCache();
    let calls = 0;

    await getChartPreview(PROJECT_A, DOCUMENT, async () => {
      calls += 1;
      return { kind: "empty" };
    });

    invalidateChartPreviewCacheForDatabase(DOCUMENT.databaseId);

    await getChartPreview(PROJECT_A, DOCUMENT, async () => {
      calls += 1;
      return { kind: "empty" };
    });

    expect(calls).toBe(2);
  });

  it.each(["old-first", "new-first"] as const)(
    "keeps the post-invalidation request authoritative when requests settle %s",
    async (settlementOrder) => {
      clearChartPreviewCache();
      const oldRequest = deferred<ChartPreviewPayload>();
      const newRequest = deferred<ChartPreviewPayload>();
      const oldPayload: ChartPreviewPayload = { kind: "empty" };
      const newPayload: ChartPreviewPayload = {
        kind: "line",
        pair: {
          data: [{ x: 8, y: 9 }],
          xLabel: "x",
          yLabel: "y",
          xFormat: "number",
          yFormat: "number",
        },
      };
      const oldCompletion = getChartPreview(PROJECT_A, DOCUMENT, () => oldRequest.promise);

      invalidateChartPreviewCacheForDatabase(DOCUMENT.databaseId);
      let newLoaderCalls = 0;
      const newCompletion = getChartPreview(PROJECT_A, DOCUMENT, () => {
        newLoaderCalls += 1;
        return newRequest.promise;
      });
      expect(newLoaderCalls).toBe(1);

      if (settlementOrder === "old-first") {
        oldRequest.resolve(oldPayload);
        await expect(oldCompletion).resolves.toBe(oldPayload);
        expect(getCachedChartPreview(PROJECT_A, DOCUMENT)).toBeUndefined();

        let thirdLoaderCalls = 0;
        const sharedCompletion = getChartPreview(PROJECT_A, DOCUMENT, async () => {
          thirdLoaderCalls += 1;
          return oldPayload;
        });
        expect(thirdLoaderCalls).toBe(0);

        newRequest.resolve(newPayload);
        await expect(sharedCompletion).resolves.toBe(newPayload);
      } else {
        newRequest.resolve(newPayload);
        await expect(newCompletion).resolves.toBe(newPayload);
        oldRequest.resolve(oldPayload);
        await expect(oldCompletion).resolves.toBe(oldPayload);
      }

      await expect(newCompletion).resolves.toBe(newPayload);
      expect(getCachedChartPreview(PROJECT_A, DOCUMENT)).toBe(newPayload);
    },
  );

  it("prevents an invalidated in-flight request from writing without a replacement", async () => {
    clearChartPreviewCache();
    const oldRequest = deferred<ChartPreviewPayload>();
    const oldPayload: ChartPreviewPayload = { kind: "empty" };
    const oldCompletion = getChartPreview(PROJECT_A, DOCUMENT, () => oldRequest.promise);

    invalidateChartPreviewCacheForDatabase(DOCUMENT.databaseId);
    oldRequest.resolve(oldPayload);

    await expect(oldCompletion).resolves.toBe(oldPayload);
    expect(getCachedChartPreview(PROJECT_A, DOCUMENT)).toBeUndefined();
  });

  it("invalidates both opaque chart path owners during a move", async () => {
    clearChartPreviewCache();
    const from = "opaque chart::before";
    const to = "opaque chart::after";
    const preview: ChartPreviewPayload = { kind: "empty" };
    await getChartPreviewForPath(PROJECT_A, from, DOCUMENT, async () => preview);
    await getChartPreviewForPath(PROJECT_A, to, DOCUMENT, async () => preview);

    invalidateChartPreviewCacheForMove(PROJECT_A, from, to);

    expect(getCachedChartPreviewForPath(PROJECT_A, from, DOCUMENT)).toBeUndefined();
    expect(getCachedChartPreviewForPath(PROJECT_A, to, DOCUMENT)).toBeUndefined();
    const snapshot = getChartPreviewCacheSnapshotForTests();
    expect(snapshot.databaseKeys.get(DOCUMENT.databaseId)?.size ?? 0).toBe(0);
    expect(snapshot.chartKeys.size).toBe(0);
    expect(snapshot.keyOwnerKeys.size).toBe(0);
  });

  it("bounds reverse ownership indexes with primary cache eviction", async () => {
    clearChartPreviewCache();
    for (let index = 0; index < 96; index += 1) {
      await getChartPreviewForPath(
        PROJECT_A,
        `opaque chart owner ${index}`,
        { ...DOCUMENT, databaseId: `database-${index}` },
        async () => ({ kind: "empty" }),
      );
    }

    const snapshot = getChartPreviewCacheSnapshotForTests();
    const databaseReferences = [...snapshot.databaseKeys.values()].reduce(
      (count, keys) => count + keys.size,
      0,
    );
    const chartReferences = [...snapshot.chartKeys.values()].reduce(
      (count, keys) => count + keys.size,
      0,
    );
    expect(snapshot.previewKeys.size).toBe(32);
    expect(snapshot.databaseKeys.size).toBe(32);
    expect(snapshot.chartKeys.size).toBe(32);
    expect(snapshot.keyOwnerKeys.size).toBe(32);
    expect(databaseReferences).toBe(32);
    expect(chartReferences).toBe(32);
    expect(snapshot.keyOwnerKeys).toEqual(snapshot.previewKeys);
  });

  it("does not reuse a completed preview across replacement projects with the same IDs", async () => {
    clearChartPreviewCache();
    const original: ChartPreviewPayload = {
      kind: "scatter",
      pair: {
        data: [{ x: 1, y: 1 }],
        xLabel: "x",
        yLabel: "y",
        xFormat: "number",
        yFormat: "number",
      },
    };
    const replacement: ChartPreviewPayload = {
      kind: "scatter",
      pair: {
        data: [{ x: 2, y: 2 }],
        xLabel: "x",
        yLabel: "y",
        xFormat: "number",
        yFormat: "number",
      },
    };

    await getChartPreview(PROJECT_A, DOCUMENT, async () => original);
    const result = await getChartPreview(PROJECT_B, DOCUMENT, async () => replacement);

    expect(result).toBe(replacement);
    expect(getCachedChartPreview(PROJECT_A, DOCUMENT)).toBe(original);
    expect(getCachedChartPreview(PROJECT_B, DOCUMENT)).toBe(replacement);
  });

  it("does not reuse an in-flight preview across replacement projects with the same IDs", async () => {
    clearChartPreviewCache();
    const oldRequest = deferred<ChartPreviewPayload>();
    const replacement: ChartPreviewPayload = {
      kind: "line",
      pair: {
        data: [{ x: 2, y: 3 }],
        xLabel: "x",
        yLabel: "y",
        xFormat: "number",
        yFormat: "number",
      },
    };
    const oldCompletion = getChartPreview(PROJECT_A, DOCUMENT, () => oldRequest.promise);

    const replacementResult = await getChartPreview(PROJECT_B, DOCUMENT, async () => replacement);
    oldRequest.resolve({ kind: "empty" });
    await oldCompletion;

    expect(replacementResult).toBe(replacement);
    expect(getCachedChartPreview(PROJECT_B, DOCUMENT)).toBe(replacement);
  });

  it("keeps a newer same-key request authoritative when a cleared request resolves", async () => {
    clearChartPreviewCache();
    const oldRequest = deferred<ChartPreviewPayload>();
    const newRequest = deferred<ChartPreviewPayload>();
    const oldPayload: ChartPreviewPayload = { kind: "empty" };
    const newPayload: ChartPreviewPayload = {
      kind: "line",
      pair: {
        data: [{ x: 4, y: 5 }],
        xLabel: "x",
        yLabel: "y",
        xFormat: "number",
        yFormat: "number",
      },
    };
    const oldCompletion = getChartPreview(PROJECT_A, DOCUMENT, () => oldRequest.promise);

    clearChartPreviewCache();
    const newCompletion = getChartPreview(PROJECT_A, DOCUMENT, () => newRequest.promise);
    oldRequest.resolve(oldPayload);
    await oldCompletion;

    expect(getCachedChartPreview(PROJECT_A, DOCUMENT)).toBeUndefined();
    let replacementLoaderCalls = 0;
    const sharedCompletion = getChartPreview(PROJECT_A, DOCUMENT, async () => {
      replacementLoaderCalls += 1;
      return oldPayload;
    });
    expect(replacementLoaderCalls).toBe(0);

    newRequest.resolve(newPayload);
    await expect(newCompletion).resolves.toBe(newPayload);
    await expect(sharedCompletion).resolves.toBe(newPayload);
    expect(getCachedChartPreview(PROJECT_A, DOCUMENT)).toBe(newPayload);
  });

  it("keeps a newer same-key request authoritative when a cleared request rejects", async () => {
    clearChartPreviewCache();
    const oldRequest = deferred<ChartPreviewPayload>();
    const newRequest = deferred<ChartPreviewPayload>();
    const newPayload: ChartPreviewPayload = {
      kind: "scatter",
      pair: {
        data: [{ x: 6, y: 7 }],
        xLabel: "x",
        yLabel: "y",
        xFormat: "number",
        yFormat: "number",
      },
    };
    const oldCompletion = getChartPreview(PROJECT_A, DOCUMENT, () => oldRequest.promise);

    clearChartPreviewCache();
    const newCompletion = getChartPreview(PROJECT_A, DOCUMENT, () => newRequest.promise);
    oldRequest.reject(new Error("stale request failed"));
    await expect(oldCompletion).rejects.toThrow("stale request failed");

    expect(getCachedChartPreview(PROJECT_A, DOCUMENT)).toBeUndefined();
    let replacementLoaderCalls = 0;
    const sharedCompletion = getChartPreview(PROJECT_A, DOCUMENT, async () => {
      replacementLoaderCalls += 1;
      return { kind: "empty" };
    });
    expect(replacementLoaderCalls).toBe(0);

    newRequest.resolve(newPayload);
    await expect(newCompletion).resolves.toBe(newPayload);
    await expect(sharedCompletion).resolves.toBe(newPayload);
    expect(getCachedChartPreview(PROJECT_A, DOCUMENT)).toBe(newPayload);
  });
});
