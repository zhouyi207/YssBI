import { describe, it, expect } from "vitest";
import { pinTypeLabel, dataTypeToThemePinType, scalarPinInputKey } from "./pinSemantics";

describe("pinSemantics", () => {
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

  it("maps scalar dataType kinds to pin input keys", () => {
    expect(scalarPinInputKey({ kind: "Scalar", inner: "Numeric" })).toBe("number");
    expect(scalarPinInputKey({ kind: "DataFrame" })).toBeNull();
  });
});
