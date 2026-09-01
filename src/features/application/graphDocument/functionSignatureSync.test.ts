import { beforeEach, describe, expect, it } from "vitest";
import { createDataSignaturePin } from "@/shared/types/domain/functionSignaturePin";
import { useGraphMetaStore } from "@/features/core/dataStore/graphMetaStore";
import {
  syncFunctionSignatureFromGraph,
  hydrateFunctionSignaturesFromProjectIndex,
} from "./functionSignatureSync";

describe("functionSignatureSync", () => {
  beforeEach(() => {
    useGraphMetaStore.setState({ graphs: {} });
  });

  it("writes function signature fields into graph meta store", () => {
    syncFunctionSignatureFromGraph({
      path: "function-1",
      name: "Compute",
      type: "function",
      functionInputs: [createDataSignaturePin("input-1", "Value", { kind: "Int64" })],
      functionOutputs: [createDataSignaturePin("output-1", "Result", { kind: "Float64" })],
    });

    expect(useGraphMetaStore.getState().graphs["function-1"]).toEqual(
      expect.objectContaining({
        functionInputs: [createDataSignaturePin("input-1", "Value", { kind: "Int64" })],
        functionOutputs: [createDataSignaturePin("output-1", "Result", { kind: "Float64" })],
      }),
    );
  });

  it("ignores older rows but repairs an incoherent equal-revision projection group", () => {
    const path = "functions/Monotonic.yssbi-function";
    const authoritativeRow = {
      path,
      name: "Monotonic",
      type: "function" as const,
      revision: 9,
      functionRevision: 9,
      functionSignature: {
        parameters: [{ id: "current", name: "Current", type_name: "Object" }],
        return_type: "Object",
      },
      functionEditorProjection: {
        functionRevision: 9,
        inputs: [
          {
            id: "current",
            name: "Observed value",
            dataType: { kind: "Struct" as const, inner: "ObservedModel" },
          },
        ],
        outputs: [
          {
            id: "computed",
            name: "Computed value",
            dataType: { kind: "Struct" as const, inner: "RegressionModel" },
          },
        ],
      },
    };
    hydrateFunctionSignaturesFromProjectIndex([authoritativeRow]);

    hydrateFunctionSignaturesFromProjectIndex([
      {
        ...authoritativeRow,
        revision: 8,
        functionRevision: 8,
        functionSignature: { parameters: [], return_type: null },
        functionEditorProjection: { functionRevision: 8, inputs: [], outputs: [] },
      },
    ]);
    expect(useGraphMetaStore.getState().graphs[path]).toMatchObject({
      functionRevision: 9,
      functionSignature: authoritativeRow.functionSignature,
      functionInputs: authoritativeRow.functionEditorProjection.inputs,
      functionOutputs: authoritativeRow.functionEditorProjection.outputs,
    });

    useGraphMetaStore.getState().updateGraph(path, {
      functionRevision: 9,
      functionSignature: { parameters: [], return_type: "Int64" },
      functionInputs: [],
      functionOutputs: [{ id: "return", name: "Result", dataType: { kind: "Int64" } }],
    });
    hydrateFunctionSignaturesFromProjectIndex([authoritativeRow]);

    expect(useGraphMetaStore.getState().graphs[path]).toMatchObject({
      functionRevision: 9,
      functionSignature: authoritativeRow.functionSignature,
      functionInputs: authoritativeRow.functionEditorProjection.inputs,
      functionOutputs: authoritativeRow.functionEditorProjection.outputs,
    });
  });

  it("installs authoritative function editor projection pins from project index rows", () => {
    hydrateFunctionSignaturesFromProjectIndex([
      {
        path: "functions/Add.yssbi-function",
        name: "Add",
        type: "function",
        revision: 7,
        functionRevision: 7,
        functionSignature: {
          parameters: [{ id: "a", name: "A", type_name: "Int64" }],
          return_type: "Object",
        },
        functionEditorProjection: {
          functionRevision: 7,
          inputs: [{ id: "a", name: "Observed value", dataType: { kind: "Float64" } }],
          outputs: [
            {
              id: "computed",
              name: "Computed value",
              dataType: { kind: "Struct", inner: "RegressionModel" },
            },
          ],
        },
      },
      { path: "events/Main.yssbi-event", name: "Main", type: "event", revision: 0 },
    ]);

    expect(useGraphMetaStore.getState().graphs["functions/Add.yssbi-function"]).toEqual(
      expect.objectContaining({
        functionRevision: 7,
        functionSignature: {
          parameters: [{ id: "a", name: "A", type_name: "Int64" }],
          return_type: "Object",
        },
        functionInputs: [{ id: "a", name: "Observed value", dataType: { kind: "Float64" } }],
        functionOutputs: [
          {
            id: "computed",
            name: "Computed value",
            dataType: { kind: "Struct", inner: "RegressionModel" },
          },
        ],
      }),
    );
    expect(useGraphMetaStore.getState().graphs["events/Main.yssbi-event"]).toBeUndefined();
  });
});
