import { resultSessionFixture, resultReferenceFixture } from "@/tests/helpers/resultFixture";
import { readFileSync } from "node:fs";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ResultDescriptor } from "@/shared/types/domain/result";
import { loadPresentationWindow } from "./loadPresentationWindow";

vi.mock("@/services/result/resultService", () => ({
  ResultService: {
    getDescriptor: vi.fn(),
    getValue: vi.fn(),
    getPage: vi.fn(),
  },
}));

vi.mock("@/utils/frontendLogger", () => ({
  logger: { app: { error: vi.fn() } },
}));

import { ResultService } from "@/services/result/resultService";

const provenance = {
  runId: "1",
  graphPath: "events/Main.yssbi-event",
  nodeId: "00000000-0000-0000-0000-000000000002",
  output: null,
  createdAtMs: "1755072000000",
};

function descriptor(resultId: string, partial: Partial<ResultDescriptor> = {}): ResultDescriptor {
  return {
    resultId,
    executionSessionId: resultSessionFixture,
    provenance,
    presentation: { kind: "inspector" },
    valueKind: "scalar",
    metadata: null,
    totalCount: 1,
    title: "Result",
    ...partial,
  };
}

describe("loadPresentationWindow", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it.each(["linearRegressionSummary", "structured"] as const)(
    "loads a ready %s result as the canonical object only",
    async (kind) => {
      const report =
        kind === "linearRegressionSummary"
          ? {
              ...JSON.parse(
                readFileSync(
                  "src/tests/fixtures/node-system-contracts/ols-summary-report.json",
                  "utf8",
                ),
              ),
              resultRef: resultReferenceFixture("20"),
            }
          : { title: "Summary", model_basic_info: {} };
      vi.mocked(ResultService.getDescriptor).mockResolvedValue(
        descriptor("20", {
          presentation: { kind: "report", report: kind },
        }),
      );
      vi.mocked(ResultService.getValue).mockResolvedValue({ kind: "value", value: report });

      await expect(loadPresentationWindow(resultReferenceFixture("20"))).resolves.toMatchObject({
        status: "ready",
        payload: {
          mode: "report",
          data: report,
          validation: { ok: true, value: { kind, data: report } },
        },
      });
      expect(ResultService.getValue).toHaveBeenCalledWith(resultReferenceFixture("20"));
      expect(ResultService.getPage).not.toHaveBeenCalled();
      vi.mocked(ResultService.getValue).mockResolvedValue({
        kind: "value",
        value:
          kind === "linearRegressionSummary"
            ? { ...report, resultRef: resultReferenceFixture("21") }
            : [],
      });
      await expect(loadPresentationWindow(resultReferenceFixture("20"))).resolves.toMatchObject({
        status: "ready",
        payload: {
          mode: "report",
          validation: {
            ok: false,
            diagnostic: { fieldPath: kind === "linearRegressionSummary" ? "resultRef" : "$" },
          },
        },
      });
    },
  );

  it.each([
    { kind: "report", report: "linearRegressionSummary" },
    { kind: "plot", chart: "scatter" },
  ] as const)("requires complete scalar payloads for $kind presentations", async (presentation) => {
    vi.mocked(ResultService.getDescriptor).mockResolvedValue(
      descriptor("21", {
        presentation,
        valueKind: "sequence",
      }),
    );
    vi.mocked(ResultService.getPage).mockResolvedValue({
      resultId: "21",
      offset: 0,
      requestedLimit: 200,
      actualCount: 1,
      totalCount: 1,
      hasMore: false,
      nextOffset: null,
      valueKind: "sequence",
      metadata: null,
      values: [{ title: "Linear Regression Summary" }],
    });

    await expect(loadPresentationWindow(resultReferenceFixture("21"))).resolves.toEqual({
      status: "load_failed",
    });
    expect(ResultService.getPage).not.toHaveBeenCalled();
  });

  it("leaves inspector payload loading to its mounted renderer", async () => {
    vi.mocked(ResultService.getDescriptor).mockResolvedValue(
      descriptor("22", {
        valueKind: "sequence",
        totalCount: 2,
      }),
    );
    vi.mocked(ResultService.getPage).mockResolvedValue({
      resultId: "22",
      offset: 0,
      requestedLimit: 200,
      actualCount: 2,
      totalCount: 2,
      hasMore: false,
      nextOffset: null,
      valueKind: "sequence",
      metadata: { columns: [{ name: "value", type: "Numeric" }] },
      values: [[1], [2]],
    });

    await expect(loadPresentationWindow(resultReferenceFixture("22"))).resolves.toMatchObject({
      status: "ready",
    });
    expect(ResultService.getValue).not.toHaveBeenCalled();
    expect(ResultService.getPage).not.toHaveBeenCalled();
  });

  it("delivers validated plot payloads and preserves the invalid-plot state", async () => {
    const plot = { data: [{ x: 1, y: 2 }] };
    vi.mocked(ResultService.getDescriptor).mockResolvedValue(
      descriptor("23", { presentation: { kind: "plot", chart: "scatter" } }),
    );
    vi.mocked(ResultService.getValue).mockResolvedValue({ kind: "value", value: plot });
    await expect(loadPresentationWindow(resultReferenceFixture("23"))).resolves.toMatchObject({
      status: "ready",
      payload: { mode: "plot", plot: { kind: "scatter", data: plot } },
    });

    vi.mocked(ResultService.getValue).mockResolvedValue({ kind: "value", value: { data: [] } });
    await expect(loadPresentationWindow(resultReferenceFixture("23"))).resolves.toMatchObject({
      status: "ready",
      payload: { mode: "plot", plot: null },
    });
  });

  it("returns explicit missing states", async () => {
    await expect(loadPresentationWindow(resultReferenceFixture(""))).resolves.toEqual({
      status: "missing_result_id",
    });
    vi.mocked(ResultService.getDescriptor).mockResolvedValue(null);
    await expect(loadPresentationWindow(resultReferenceFixture("999"))).resolves.toEqual({
      status: "not_found",
    });
  });
});
