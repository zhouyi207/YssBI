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
    expect(createDataSignaturePin("a", "A", { kind: "Scalar", inner: "Numeric" })).toEqual({
      id: "a",
      name: "A",
      dataType: { kind: "Scalar", inner: "Numeric" },
    });
  });

  it("builds container types from scalar + overlay", () => {
    expect(buildSignatureDataType("Numeric", "dataseries")).toEqual({
      kind: "DataSeries",
      inner: { kind: "Scalar", inner: "Numeric" },
    });
  });

  it("cycles container overlay on data pins", () => {
    const pin = {
      id: "a",
      name: "V",
      dataType: { kind: "Scalar" as const, inner: "Numeric" as const },
    };
    const withSeries = cycleSignatureContainer(pin);
    expect(withSeries.dataType).toEqual({
      kind: "DataSeries",
      inner: { kind: "Scalar", inner: "Numeric" },
    });
    const scalar = cycleSignatureContainer(withSeries);
    expect(scalar.dataType).toEqual({ kind: "Scalar", inner: "Numeric" });
  });

  it("maps editor type options to structured dataType", () => {
    const pin = {
      id: "a",
      name: "V",
      dataType: { kind: "Scalar" as const, inner: "Numeric" as const },
    };
    expect(applySignatureEditorType(pin, "Numeric").dataType).toEqual({
      kind: "Scalar",
      inner: "Numeric",
    });
    expect(signatureEditorTypeOption(pin)).toBe("Numeric");
  });
});
