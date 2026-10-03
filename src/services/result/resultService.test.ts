import {
  resultSessionFixture,
  resultReferenceFixture,
  resultLeaseIdFixture,
} from "@/tests/helpers/resultFixture";
import { invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { PortAddressDto } from "@/shared/types/dto/editorProjection";
import {
  parseResultDescriptor,
  parseResultPage,
  parseResultValue,
  parseGraphResultState,
} from "@/shared/types/dto/resultParser";
import { ResultService } from "./resultService";
import { isResultReference } from "@/shared/types/domain/result";
import { resultReferenceField } from "@/shared/types/report/parseLinearRegression";
import executionFixture from "@/tests/fixtures/node-system-contracts/execution-wire.json";

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
  presentation: { kind: "report" as const, report: "linearRegressionSummary" as const },
  valueKind: "scalar" as const,
  metadata: null,
  totalCount: 1,
  title: "Linear Regression Summary",
};

describe("result DTO parsers", () => {
  it("shares the Rust result identity range across references and result responses", () => {
    const page = {
      resultId: "17",
      offset: 0,
      requestedLimit: 1,
      actualCount: 0,
      totalCount: 0,
      hasMore: false,
      nextOffset: null,
      valueKind: "scalar",
      metadata: null,
      values: [],
    };
    const graphState = (resultId: string) => ({
      ...executionFixture.graphResultState,
      outputs: executionFixture.graphResultState.outputs.map((entry, index) =>
        index === 0 ? { ...entry, resultId } : entry,
      ),
    });
    for (const resultId of ["1", "9007199254740993", "18446744073709551615"]) {
      const reference = resultReferenceFixture(resultId);
      expect(isResultReference(reference)).toBe(true);
      expect(resultReferenceField.read(reference, "$").ok).toBe(true);
      expect(parseResultDescriptor({ ...readyDescriptor, resultId }).resultId).toBe(resultId);
      expect(parseResultPage({ ...page, resultId }).resultId).toBe(resultId);
      expect(parseGraphResultState(graphState(resultId)).outputs[0].resultId).toBe(resultId);
    }
    for (const resultId of ["0", "01", "-1", "", "18446744073709551616", "1".repeat(100)]) {
      const reference = resultReferenceFixture(resultId);
      expect.soft(isResultReference(reference)).toBe(false);
      expect.soft(resultReferenceField.read(reference, "$").ok).toBe(false);
      expect.soft(() => parseResultDescriptor({ ...readyDescriptor, resultId })).toThrow();
      expect.soft(() => parseResultPage({ ...page, resultId })).toThrow();
      expect.soft(() => parseGraphResultState(graphState(resultId))).toThrow();
    }
  });

  it("parses available result descriptors and rejects unrecognized fields", () => {
    expect(parseResultDescriptor(readyDescriptor)).toEqual(readyDescriptor);
    const structured = {
      ...readyDescriptor,
      presentation: { kind: "report", report: "structured" },
    };
    expect(parseResultDescriptor(structured)).toEqual(structured);
    expect(() => parseResultDescriptor({ ...readyDescriptor, extra: true })).toThrow();
    expect(() =>
      parseResultDescriptor({
        ...readyDescriptor,
        provenance: { ...provenance, graphPath: "events/other.yssbi-event" },
      }),
    ).toThrow();
    expect(() =>
      parseResultDescriptor({
        ...readyDescriptor,
        provenance: { ...provenance, nodeId: resultSessionFixture },
      }),
    ).toThrow();
    expect(
      parseResultDescriptor({ ...readyDescriptor, provenance: { ...provenance, output: null } }),
    ).toMatchObject({ provenance: { output: null } });
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

    const page = {
      resultId: "17",
      offset: 0,
      requestedLimit: 2,
      actualCount: 2,
      totalCount: 3,
      hasMore: true,
      nextOffset: 2,
      valueKind: "sequence" as const,
      metadata: { columns: [{ name: "x", type: "Numeric" }] },
      values: [[1], [null]],
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

  it("reads graph cache identities and rejects invalid, duplicate, or misrouted outputs", async () => {
    const state = executionFixture.graphResultState;
    const hash = state.semanticInputHash;
    const graphPath = state.outputs[0].output.graphPath;
    vi.mocked(invoke).mockResolvedValueOnce(state);
    await expect(ResultService.getGraphState(graphPath, hash)).resolves.toEqual(state);
    expect(invoke).toHaveBeenCalledWith("get_graph_result_state", {
      graphPath,
      semanticInputHash: hash,
    });
    expect(() =>
      parseGraphResultState({ ...state, outputs: [{ ...state.outputs[0], resultId: null }] }),
    ).toThrow();
    expect(() =>
      parseGraphResultState({ ...state, outputs: [{ ...state.outputs[0], state: "stale" }] }),
    ).toThrow();
    expect(() =>
      parseGraphResultState({
        ...state,
        connections: [...state.connections, state.connections[0]],
      }),
    ).toThrow();
    expect(() =>
      parseGraphResultState({ ...state, connections: [{ ...state.connections[0], input: {} }] }),
    ).toThrow();
    expect(() =>
      parseGraphResultState({
        ...state,
        connections: [{ ...state.connections[0], state: "completed" }],
      }),
    ).toThrow();
    expect(() =>
      parseGraphResultState({
        ...state,
        outputs: [
          ...state.outputs,
          {
            ...state.outputs[0],
            output: {
              graphPath,
              port: { portKey: output.portKey, nodeId: output.nodeId, kind: output.kind },
            },
          },
        ],
      }),
    ).toThrow();
    vi.mocked(invoke).mockResolvedValueOnce(state);
    await expect(ResultService.getGraphState("events/other.yssbi-event", hash)).rejects.toThrow(
      "Mismatched graph result state",
    );
  });

  it("uses session-bound references for report tables and typed statistical analysis", async () => {
    const reference = {
      executionSessionId: "00000000-0000-0000-0000-000000000001",
      resultId: "17",
    };
    await ResultService.getPage(resultReferenceFixture("17"), 200, 200, "observations");
    await ResultService.getPage(resultReferenceFixture("17"), 53930, 100, "structured:/fitted");
    const response = {
      kind: "acfPacf",
      value: { acf: [1, 0.5], pacf: [0.5], n: 53940, ciHalfWidth: 0.00844 },
    };
    vi.mocked(invoke).mockResolvedValueOnce(response);
    await expect(ResultService.analyze(reference, { kind: "acfPacf" })).resolves.toEqual(response);
    expect(vi.mocked(invoke).mock.calls).toEqual([
      ["get_result_table_page", { reference, part: "observations", offset: 200, limit: 200 }],
      [
        "get_result_table_page",
        { reference, part: "structured:/fitted", offset: 53930, limit: 100 },
      ],
      ["analyze_result", { reference, analysis: { kind: "acfPacf" } }],
    ]);
    vi.mocked(invoke).mockResolvedValueOnce({
      ...response,
      value: { ...response.value, acf: ["invalid"] },
    });
    await expect(ResultService.analyze(reference, { kind: "acfPacf" })).rejects.toThrow(
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

  it("rejects descriptors, output lookups and claimed leases belonging to another request", async () => {
    const reference = resultReferenceFixture("17");
    vi.mocked(invoke).mockResolvedValueOnce({ ...readyDescriptor, resultId: "18" });
    await expect(ResultService.getDescriptor(reference)).rejects.toThrow();
    vi.mocked(invoke).mockResolvedValueOnce({
      ...readyDescriptor,
      executionSessionId: resultLeaseIdFixture(2),
    });
    await expect(ResultService.getDescriptor(reference)).rejects.toThrow();

    vi.mocked(invoke).mockResolvedValueOnce(readyDescriptor);
    await expect(ResultService.getPinResult("events/other.yssbi-event", output)).rejects.toThrow();
    vi.mocked(invoke).mockResolvedValueOnce(readyDescriptor);
    await expect(
      ResultService.getPinResult(provenance.graphPath, { ...output, portKey: "other" }),
    ).rejects.toThrow();
    vi.mocked(invoke).mockResolvedValueOnce({
      ...readyDescriptor,
      provenance: { ...provenance, output: null },
    });
    await expect(ResultService.getPinResult(provenance.graphPath, output)).rejects.toThrow();

    const leaseId = resultLeaseIdFixture(3);
    vi.mocked(invoke).mockResolvedValueOnce({
      leaseId: resultLeaseIdFixture(4),
      descriptor: readyDescriptor,
    });
    await expect(ResultService.claim(leaseId)).rejects.toThrow();
    vi.mocked(invoke).mockResolvedValueOnce({ leaseId, descriptor: readyDescriptor });
    await expect(ResultService.claim(leaseId)).resolves.toEqual({
      leaseId,
      descriptor: readyDescriptor,
    });
    expect(invoke).toHaveBeenLastCalledWith("claim_result_lease", { lease: leaseId });
  });

  it("binds pages to the requested result and range while preserving end-of-data offsets", async () => {
    const reference = resultReferenceFixture("17");
    const page = {
      resultId: "17",
      offset: 2,
      requestedLimit: 2,
      actualCount: 2,
      totalCount: 6,
      hasMore: true,
      nextOffset: 4,
      valueKind: "sequence",
      metadata: { columns: [{ name: "x", type: "Numeric" }] },
      values: [[2], [3]],
    };
    vi.mocked(invoke).mockResolvedValueOnce({ ...page, resultId: "18" });
    await expect(ResultService.getPage(reference, 2, 2)).rejects.toThrow();
    vi.mocked(invoke).mockResolvedValueOnce({ ...page, offset: 1, nextOffset: 3 });
    await expect(ResultService.getPage(reference, 2, 2)).rejects.toThrow();
    vi.mocked(invoke).mockResolvedValueOnce({ ...page, requestedLimit: 3 });
    await expect(ResultService.getPage(reference, 2, 2, "observations")).rejects.toThrow();

    vi.mocked(invoke).mockResolvedValueOnce(page);
    await expect(ResultService.getPage(reference, 2, 2)).resolves.toEqual(page);
    const end = {
      ...page,
      offset: 6,
      actualCount: 0,
      hasMore: false,
      nextOffset: null,
      values: [],
    };
    vi.mocked(invoke).mockResolvedValueOnce(end);
    await expect(ResultService.getPage(reference, 12, 2, "observations")).resolves.toEqual(end);
    const unknownEnd = { ...end, offset: 12, totalCount: null };
    vi.mocked(invoke).mockResolvedValueOnce(unknownEnd);
    await expect(ResultService.getPage(reference, 12, 2)).resolves.toEqual(unknownEnd);
  });
});
