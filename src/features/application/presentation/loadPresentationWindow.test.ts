import { resultSessionFixture, resultReferenceFixture } from "@/tests/helpers/resultFixture";
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

vi.mock("@/features/application/observability/appLogger", () => ({
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

  it("loads a ready scalar report as the canonical object only", async () => {
    const report = { title: "OLS Summary", model_basic_info: {} };
    vi.mocked(ResultService.getDescriptor).mockResolvedValue(
      descriptor("20", {
        presentation: { kind: "report", report: "olsSummary" },
      }),
    );
    vi.mocked(ResultService.getValue).mockResolvedValue({ kind: "value", value: report });

    await expect(loadPresentationWindow(resultReferenceFixture("20"))).resolves.toMatchObject({
      status: "ready",
      payload: { mode: "report", report: "olsSummary", data: report },
    });
    expect(ResultService.getValue).toHaveBeenCalledWith(resultReferenceFixture("20"));
    expect(ResultService.getPage).not.toHaveBeenCalled();
  });

  it("requires report descriptors to use the scalar value kind", async () => {
    vi.mocked(ResultService.getDescriptor).mockResolvedValue(
      descriptor("21", {
        presentation: { kind: "report", report: "olsSummary" },
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
      values: [{ title: "OLS Summary" }],
    });

    await expect(loadPresentationWindow(resultReferenceFixture("21"))).resolves.toEqual({
      status: "load_failed",
    });
  });

  it("leaves inspector payload loading to its mounted renderer", async () => {
    vi.mocked(ResultService.getDescriptor).mockResolvedValue(
      descriptor("22", {
        valueKind: "dataSeries",
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
      valueKind: "dataSeries",
      metadata: null,
      values: [1, 2],
    });

    await expect(loadPresentationWindow(resultReferenceFixture("22"))).resolves.toMatchObject({
      status: "ready",
    });
    expect(ResultService.getValue).not.toHaveBeenCalled();
    expect(ResultService.getPage).not.toHaveBeenCalled();
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
