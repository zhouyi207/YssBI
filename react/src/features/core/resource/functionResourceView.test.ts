import { describe, expect, it } from "vitest";
import { createDataSignaturePin } from "@/shared/types/domain/functionSignaturePin";
import { buildFunctionResourceView } from "./functionResourceView";

describe("functionResourceView", () => {
  it("merges resource name with graph meta signature", () => {
    const view = buildFunctionResourceView(
      { id: "functions/Add.yssbi-function", name: "Add" },
      {
        functionInputs: [createDataSignaturePin("in-1", "A", { kind: "Scalar", inner: "Numeric" })],
        functionOutputs: [
          createDataSignaturePin("out-1", "R", { kind: "Scalar", inner: "Numeric" }),
        ],
      },
    );

    expect(view).toEqual({
      id: "functions/Add.yssbi-function",
      name: "Add",
      functionInputs: [createDataSignaturePin("in-1", "A", { kind: "Scalar", inner: "Numeric" })],
      functionOutputs: [createDataSignaturePin("out-1", "R", { kind: "Scalar", inner: "Numeric" })],
    });
  });
});
