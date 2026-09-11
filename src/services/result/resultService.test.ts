import { resultSessionFixture, resultReferenceFixture } from "@/tests/helpers/resultFixture";
import { invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { PortAddressDto } from "@/shared/types/dto/editorProjection";
import {
  parseResultDescriptor,
  parseResultPage,
  parseResultValue,
} from "@/shared/types/dto/resultParser";
import { ResultService } from "./resultService";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const output: PortAddressDto = {
  kind: "declared",
  nodeId: "00000000-0000-0000-0000-000000000002",
  portKey: "result",
};

const provenance = {
  runId: "9007199254740993",
  graphPath: "events/contract.yssbi-event",
  nodeId: "00000000-0000-0000-0000-000000000002",
  output: { graphPath: "events/contract.yssbi-event", port: output },
  createdAtMs: "1755072000000",
};

const readyDescriptor = {
  resultId: "17",

  executionSessionId: resultSessionFixture,
  provenance,
  presentation: { kind: "report" as const, report: "olsSummary" as const },
  valueKind: "scalar" as const,
  metadata: null,
  totalCount: 1,
  title: "OLS Summary",
};

describe("result DTO parsers", () => {
  it("parses available result descriptors and rejects unrecognized fields", () => {
    expect(parseResultDescriptor(readyDescriptor)).toEqual(readyDescriptor);
    expect(() => parseResultDescriptor({ ...readyDescriptor, extra: true })).toThrow();
  });

  it("strictly parses value, page, and metadata variants", () => {
    expect(parseResultValue({ kind: "value", value: { report: true } })).toEqual({
      kind: "value",
      value: { report: true },
    });
    expect(parseResultValue({ kind: "sequence", value: [1, 2] })).toEqual({
      kind: "sequence",
      value: [1, 2],
    });
    expect(parseResultValue({ kind: "dataSeries", value: [1, null] })).toEqual({
      kind: "dataSeries",
      value: [1, null],
    });

    const page = {
      resultId: "17",
      offset: 0,
      requestedLimit: 2,
      actualCount: 2,
      totalCount: 3,
      hasMore: true,
      nextOffset: 2,
      valueKind: "dataSeries" as const,
      metadata: {
        elementType: "float64" as const,
        length: 3,
        nullCount: 1,
        name: "x",
        format: null,
      },
      values: [1, null],
    };
    expect(parseResultPage(page)).toEqual(page);

    expect(() => parseResultPage({ ...page, limit: 2 })).toThrow();
    const table = {
      ...page,
      totalCount: null,
      valueKind: "sequence",
      metadata: { columns: [{ name: "id", type: "UInt64" }] },
      values: [["18446744073709551615"], [null]],
    };
    expect(parseResultPage(table)).toEqual(table);
    expect(() => parseResultPage({ ...table, nextOffset: 0 })).toThrow();
    expect(() => parseResultPage({ ...table, actualCount: 1 })).toThrow();
    expect(() => parseResultPage({ ...table, values: [[1, 2], [null]] })).toThrow();
  });
});

describe("ResultService", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(invoke).mockResolvedValue(null);
  });

  it("uses session-bound references for report tables and typed statistical analysis", async () => {
    const reference = {
      executionSessionId: "00000000-0000-0000-0000-000000000001",
      resultId: "17",
    };
    await ResultService.getPage(resultReferenceFixture("17"), 200, 200, "observations");
    const response = { kind: "acfPacf", value: { acf: [1, 0.5], pacf: [0.5], n: 53940 } };
    vi.mocked(invoke).mockResolvedValueOnce(response);
    await expect(ResultService.analyze(reference, { kind: "acfPacf", maxLag: 1 })).resolves.toEqual(
      response,
    );
    expect(vi.mocked(invoke).mock.calls).toEqual([
      ["get_result_table_page", { reference, part: "observations", offset: 200, limit: 200 }],
      ["analyze_result", { reference, analysis: { kind: "acfPacf", maxLag: 1 } }],
    ]);
    vi.mocked(invoke).mockResolvedValueOnce({
      ...response,
      value: { ...response.value, acf: ["invalid"] },
    });
    await expect(ResultService.analyze(reference, { kind: "acfPacf", maxLag: 1 })).rejects.toThrow(
      "Invalid result analysis",
    );
  });

  it("invokes the exact result commands with decimal IDs and PortAddressDto output", async () => {
    vi.mocked(invoke)
      .mockResolvedValueOnce(null)
      .mockResolvedValueOnce(null)
      .mockResolvedValueOnce(null)
      .mockResolvedValueOnce(null);
    await ResultService.getDescriptor(resultReferenceFixture("17"));
    await ResultService.getValue(resultReferenceFixture("18"));
    await ResultService.getPage(resultReferenceFixture("19"), 10, 20);
    await ResultService.getPinResult("events/contract.yssbi-event", output);

    expect(vi.mocked(invoke).mock.calls).toEqual([
      ["get_result_descriptor", { reference: resultReferenceFixture("17") }],
      ["get_result_value", { reference: resultReferenceFixture("18") }],
      ["get_result_page", { reference: resultReferenceFixture("19"), offset: 10, limit: 20 }],
      ["get_pin_result", { graphPath: "events/contract.yssbi-event", output }],
    ]);
    expect(invoke).not.toHaveBeenCalledWith(expect.stringContaining("release"), expect.anything());
  });

  it("parses command responses instead of trusting unknown IPC values", async () => {
    vi.mocked(invoke)
      .mockResolvedValueOnce(readyDescriptor)
      .mockResolvedValueOnce({ kind: "value", value: 4 })
      .mockResolvedValueOnce(null)
      .mockResolvedValueOnce(readyDescriptor);

    await expect(ResultService.getDescriptor(resultReferenceFixture("17"))).resolves.toEqual(
      readyDescriptor,
    );
    await expect(ResultService.getValue(resultReferenceFixture("17"))).resolves.toEqual({
      kind: "value",
      value: 4,
    });
    await expect(ResultService.getPage(resultReferenceFixture("17"), 0, 10)).resolves.toBeNull();
    await expect(
      ResultService.getPinResult("events/contract.yssbi-event", output),
    ).resolves.toEqual(readyDescriptor);
  });
});
