import { describe, expect, it } from "vitest";
import { dataTypeFromBackend } from "./valueType";

describe("ValueType DTO conversion", () => {
  it("preserves Struct inner keys from backend payloads", () => {
    expect(dataTypeFromBackend({ kind: "Struct", inner: "OLSModel" })).toEqual({
      kind: "Struct",
      inner: "OLSModel",
    });
  });
});
