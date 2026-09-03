import { describe, it, expect } from "vitest";
import { pinTypeLabel, dataTypeToThemePinType, scalarPinInputKey } from "./pinSemantics";

describe("pinSemantics", () => {
  it("derives labels from structured dataType", () => {
    expect(
      pinTypeLabel({
        dataType: { kind: "DataSeries", inner: { kind: "Float64" } },
      }),
    ).toBe("DataSeries<Float64>");
  });

  it("requires structured dataType for data pin labels", () => {
    expect(pinTypeLabel({})).toBe("unknown");
  });

  it("maps structured data types to theme keys", () => {
    expect(dataTypeToThemePinType({ kind: "DataFrame" })).toBe("dataframe");
  });

  it("maps scalar dataType kinds to pin input keys", () => {
    expect(scalarPinInputKey({ kind: "Int64" })).toBe("Int64");
    expect(scalarPinInputKey({ kind: "DataFrame" })).toBeNull();
  });
});
