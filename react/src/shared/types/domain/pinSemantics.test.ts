import { describe, it, expect } from "vitest";
import {
  pinTypeLabel,
  pinTypeColorKey,
  dataTypeToThemePinType,
  scalarPinInputKey,
} from "./pinSemantics";
import type { PortTypeStateDto } from "./editorProjection";
import { dataTypeDisplay } from "./valueType";

describe("pinSemantics", () => {
  it("displays all seven meanings independently of physical representation", () => {
    for (const inner of [
      "Numeric",
      "Categorical",
      "Ordinal",
      "Binary",
      "Datetime",
      "Text",
      "Identifier",
    ] as const) {
      expect(dataTypeDisplay({ kind: "Scalar", inner })).toBe(inner);
    }
  });
  it("keeps series structure and semantic unions explicit", () => {
    expect(
      dataTypeDisplay({ kind: "DataSeries", inner: { kind: "Scalar", inner: "Numeric" } }),
    ).toBe("DataSeries<Numeric>");
    expect(
      dataTypeDisplay({
        kind: "OneOf",
        inner: [
          { kind: "Scalar", inner: "Numeric" },
          { kind: "Scalar", inner: "Text" },
        ],
      }),
    ).toBe("Numeric | Text");
  });
  it("derives labels from structured dataType", () => {
    expect(
      pinTypeLabel({
        typeState: {
          status: "exact",
          display: "ignored",
          dataType: { kind: "DataSeries", inner: { kind: "Scalar", inner: "Numeric" } },
        },
      }),
    ).toBe("DataSeries<Numeric>");
  });

  it("requires structured dataType for data pin labels", () => {
    expect(
      pinTypeLabel({ typeState: { status: "unknown", reasonCode: "unresolved_upstream" } }),
    ).toBe("unknown");
  });

  it("maps structured data types to theme keys", () => {
    expect(dataTypeToThemePinType({ kind: "DataFrame" })).toBe("dataframe");
  });

  it("uses exact nested type colors and keeps unresolved states neutral", () => {
    expect(
      pinTypeColorKey({
        typeState: {
          status: "exact",
          display: "ignored",
          dataType: {
            kind: "Array",
            inner: { kind: "DataSeries", inner: { kind: "Scalar", inner: "Numeric" } },
          },
        },
      }),
    ).toBe("numeric");
    const unresolved: PortTypeStateDto[] = [
      { status: "exact", display: "Numeric", dataType: null },
      { status: "constrained", display: "Numeric", domain: [{ kind: "Scalar", inner: "Numeric" }] },
      { status: "unknown", reasonCode: "unresolved_upstream" },
      { status: "conflict", diagnosticCode: "type_conflict" },
    ];
    for (const typeState of unresolved) expect(pinTypeColorKey({ typeState })).toBe("object");
  });

  it("maps scalar dataType kinds to pin input keys", () => {
    expect(scalarPinInputKey({ kind: "Scalar", inner: "Numeric" })).toBe("number");
    expect(scalarPinInputKey({ kind: "DataFrame" })).toBeNull();
  });
});
