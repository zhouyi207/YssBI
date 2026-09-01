import { describe, expect, it } from "vitest";
import { createDataSignaturePin } from "@/shared/types/domain/functionSignaturePin";
import { buildFunctionResourceCatalog, buildFunctionResourceView } from "./functionResourceView";

describe("functionResourceView", () => {
  it("merges resource name with graph meta signature", () => {
    const view = buildFunctionResourceView(
      "functions/Add.yssbi-function",
      { id: "functions/Add.yssbi-function", name: "Add" },
      {
        functionInputs: [createDataSignaturePin("in-1", "A", { kind: "Float64" })],
        functionOutputs: [createDataSignaturePin("out-1", "R", { kind: "Float64" })],
      },
    );

    expect(view).toEqual({
      id: "functions/Add.yssbi-function",
      name: "Add",
      functionInputs: [createDataSignaturePin("in-1", "A", { kind: "Float64" })],
      functionOutputs: [createDataSignaturePin("out-1", "R", { kind: "Float64" })],
    });
  });

  it("builds catalog by path", () => {
    const resources = { "fn-1": { id: "fn-1", name: "Add" } };
    const metaGraphs = {
      "fn-1": {
        path: "fn-1",
        name: "Add",
        type: "function" as const,
        functionInputs: [createDataSignaturePin("in-1", "A", { kind: "Int64" })],
        functionOutputs: [],
      },
    };

    expect(buildFunctionResourceCatalog(resources, metaGraphs)).toEqual({
      "fn-1": {
        id: "fn-1",
        name: "Add",
        functionInputs: [createDataSignaturePin("in-1", "A", { kind: "Int64" })],
        functionOutputs: [],
      },
    });
  });
});
