import { describe, expect, it } from "vitest";
import { createDataSignaturePin } from "@/shared/types/domain/functionSignaturePin";
import type { LogRecordDto } from "@/shared/types/domain/log";
import { resolveDetailPanelModel } from "./resolveDetailPanelModel";

const logEntry = {
  streamId: "stream-1",
  sequence: 1,
  timestamp: "2026-08-16T10:11:12.000Z",
  level: "info",
  origin: "frontend",
  domain: "application",
  target: "test",
  message: "hello",
  fields: {},
} satisfies LogRecordDto;

const targetData = {
  chartName: null,
  eventName: null,
  functionGraph: { id: "fn-1", name: "Add", functionInputs: [], functionOutputs: [] },
  dataframe: { id: "df-1", name: "Sales", rowCount: 10 },
};

describe("resolveDetailPanelModel", () => {
  it("returns empty when target is null", () => {
    expect(
      resolveDetailPanelModel({
        target: null,
        selectedLog: null,
        chartDocument: null,
        ...targetData,
      }),
    ).toEqual({ kind: "empty" });
  });

  it("resolves resource-backed panels from the selected resource", () => {
    expect(
      resolveDetailPanelModel({
        target: { kind: "data", id: "df-1" },
        selectedLog: null,
        chartDocument: null,
        ...targetData,
      }),
    ).toMatchObject({ kind: "data", id: "df-1", dataframe: { name: "Sales" } });
  });

  it("preserves the focused graph path for node detail selection", () => {
    expect(
      resolveDetailPanelModel({
        target: { kind: "node", id: "shared-node", graphPath: "functions/second" },
        selectedLog: null,
        chartDocument: null,
        ...targetData,
      }),
    ).toEqual({
      kind: "node",
      nodeId: "shared-node",
      graphPath: "functions/second",
    });
  });

  it("merges function signature pins into function panel model", () => {
    const model = resolveDetailPanelModel({
      target: { kind: "function_graph", path: "fn-1" },
      selectedLog: null,
      chartDocument: null,
      ...targetData,
      functionGraph: {
        id: "fn-1",
        name: "Add",
        functionInputs: [createDataSignaturePin("in-1", "A", { kind: "Scalar", inner: "Numeric" })],
        functionOutputs: [
          createDataSignaturePin("out-1", "R", { kind: "Scalar", inner: "Numeric" }),
        ],
      },
    });

    expect(model).toEqual({
      kind: "function_graph",
      path: "fn-1",
      fn: {
        name: "Add",
        inputs: [createDataSignaturePin("in-1", "A", { kind: "Scalar", inner: "Numeric" })],
        outputs: [createDataSignaturePin("out-1", "R", { kind: "Scalar", inner: "Numeric" })],
      },
    });
  });

  it("distinguishes unavailable resource details from no selected log", () => {
    expect(
      resolveDetailPanelModel({
        target: { kind: "event_graph", path: "missing" },
        selectedLog: null,
        chartDocument: null,
        ...targetData,
      }),
    ).toEqual({ kind: "unavailable", resourceKind: "event_graph", resourceRef: "missing" });

    expect(
      resolveDetailPanelModel({
        target: { kind: "log" },
        selectedLog: null,
        chartDocument: null,
        ...targetData,
      }),
    ).toEqual({ kind: "empty" });

    expect(
      resolveDetailPanelModel({
        target: { kind: "log" },
        selectedLog: logEntry,
        chartDocument: null,
        ...targetData,
      }),
    ).toEqual({ kind: "log", log: logEntry });
  });
});
