import { describe, expect, it } from "vitest";
import { dataTypeFromBackend, dataTypeToBackend } from "./dataType";

describe("DataType DTO conversion", () => {
  it("preserves Struct inner keys from backend payloads", () => {
    expect(dataTypeFromBackend({ kind: "Struct", inner: "OLSModel" })).toEqual({
      kind: "Struct",
      inner: "OLSModel",
    });
  });

  it("serializes Struct inner keys for backend payloads", () => {
    expect(dataTypeToBackend({ kind: "Struct", inner: "Model" })).toEqual({
      kind: "Struct",
      inner: "Model",
    });
  });
});
