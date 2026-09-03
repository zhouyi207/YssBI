import { describe, expect, it } from "vitest";
import {
  applySignatureEditorType,
  buildSignatureDataType,
  createDataSignaturePin,
  cycleSignatureContainer,
  signatureEditorTypeOption,
} from "./functionSignaturePin";

describe("functionSignaturePin", () => {
  it("creates typed data signature pins", () => {
    expect(createDataSignaturePin("a", "A", { kind: "Float64" })).toEqual({
      id: "a",
      name: "A",
      dataType: { kind: "Float64" },
    });
  });

  it("builds container types from scalar + overlay", () => {
    expect(buildSignatureDataType("Float64", "dataseries")).toEqual({
      kind: "DataSeries",
      inner: { kind: "Float64" },
    });
  });

  it("cycles container overlay on data pins", () => {
    const pin = { id: "a", name: "V", dataType: { kind: "Int64" as const } };
    const withArray = cycleSignatureContainer(pin);
    expect(withArray.dataType).toEqual({ kind: "Array", inner: { kind: "Int64" } });
    const withSeries = cycleSignatureContainer(withArray);
    expect(withSeries.dataType).toEqual({
      kind: "DataSeries",
      inner: { kind: "Int64" },
    });
    const scalar = cycleSignatureContainer(withSeries);
    expect(scalar.dataType).toEqual({ kind: "Int64" });
  });

  it("maps editor type options to structured dataType", () => {
    const pin = { id: "a", name: "V", dataType: { kind: "Int64" as const } };
    expect(applySignatureEditorType(pin, "float").dataType).toEqual({ kind: "Float64" });
    expect(signatureEditorTypeOption(pin)).toBe("int");
  });
});
