import { describe, expect, it } from "vitest";
import {
  parseReportDisplay,
  parseStructuredReportRows,
  structuredValueAt,
} from "./structuredReportDisplay";

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
    const section = parseReportDisplay(value).coefficients;
    expect(section.path).toBe("/coefficients");
    expect(section.value).toBe(value.coefficients);
    value.report_display.sections.coefficients.title = " \t";
    expect(() => parseReportDisplay(value)).toThrow("invalid_report_display");
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

  it("rejects malformed paged rows instead of silently omitting them", () => {
    const root = { re: 0.8, im: -0.2, modulus: 0.82 };
    const stability = {
      kind: "stability" as const,
      title: "Roots",
      path: "/roots",
      columns: {},
      value: { kind: "tableRef" as const, part: "structured:/roots" as const, rowCount: 2 },
    };
    const valid = parseStructuredReportRows(stability, [[root]]);
    expect(valid).toEqual({ kind: "stability", rows: [root] });
    expect(valid?.rows[0]).toBe(root);
    expect(parseStructuredReportRows(stability, [[root], [null]])).toBeNull();
    expect(parseStructuredReportRows(stability, [[{ re: Infinity, im: 0 }]])).toBeNull();

    const table = { ...stability, kind: "table" as const, columns: { re: "Real" } };
    expect(parseStructuredReportRows(table, [[root]])).toEqual({ kind: "table", rows: [root] });
    expect(parseStructuredReportRows(table, [[{ im: 0 }]])).toBeNull();
    expect(parseStructuredReportRows(table, [[{ re: {} }]])).toBeNull();
  });
});
