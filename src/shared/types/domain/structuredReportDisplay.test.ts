import { describe, expect, it } from "vitest";
import { parseReportDisplay, structuredValueAt } from "./structuredReportDisplay";

describe("structured report display boundary", () => {
  it("accepts explicit bounded result paths and rejects missing or inherited targets", () => {
    const value = {
      coefficients: { kind: "tableRef", part: "structured:/coefficients", rowCount: 3 },
      report_display: {
        sections: {
          coefficients: {
            title: "Coefficients",
            kind: "table",
            path: "/coefficients",
            columns: { variable: "Variable" },
          },
        },
      },
    };
    expect(parseReportDisplay(value).coefficients.path).toBe("/coefficients");
    expect(structuredValueAt({}, "/constructor")).toBeUndefined();
    expect(structuredValueAt({ "a/b": 2 }, "/a~1b")).toBe(2);
    expect(structuredValueAt({}, "/a~2b")).toBeUndefined();
    expect(() =>
      parseReportDisplay({
        ...value,
        report_display: {
          sections: { wrong: { title: "Wrong", kind: "equation", path: "/absent" } },
        },
      }),
    ).toThrow("invalid_report_display");
  });
});
