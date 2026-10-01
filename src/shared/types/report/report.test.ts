import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";
import { parseReportPayloadResult } from "./parseReportPayload";
import { parseResultAnalysis } from "./parseLinearRegression";

describe("parseReportPayloadResult", () => {
  it("preserves structured result fields and rejects non-object payloads", () => {
    const payload = {
      title: "Panel Summary",
      model: { method: "Fixed Effects", statistic: null },
      coefficients: { kind: "tableRef", part: "structured:/coefficients", rowCount: 3 },
    };
    expect(parseReportPayloadResult("structured", payload)).toEqual({ ok: true, value: payload });
    expect(parseReportPayloadResult("structured", [payload])).toMatchObject({
      ok: false,
      issue: { fieldPath: "$", reason: "expected structured result data" },
    });
  });

  it("accepts the Rust Linear Regression Summary report output", () => {
    const payload: unknown = JSON.parse(
      readFileSync(
        resolve("src/tests/fixtures/node-system-contracts/ols-summary-report.json"),
        "utf8",
      ),
    );

    expect(parseReportPayloadResult("linearRegressionSummary", payload)).toEqual({
      ok: true,
      value: payload,
    });
  });

  it("rejects display values and table rows that contradict their declared formats", () => {
    const read = () =>
      JSON.parse(
        readFileSync(
          resolve("src/tests/fixtures/node-system-contracts/ols-summary-report.json"),
          "utf8",
        ),
      );
    const metric = read();
    metric.presentation.summary.items[2].value = "0.875";
    expect(parseReportPayloadResult("linearRegressionSummary", metric)).toMatchObject({
      ok: false,
      issue: { fieldPath: "presentation.summary.items[2].value" },
    });
    const table = read();
    table.presentation.anova.rows[0].pop();
    expect(parseReportPayloadResult("linearRegressionSummary", table)).toMatchObject({
      ok: false,
      issue: { fieldPath: "presentation.anova.rows[0]" },
    });
  });
});

describe("parseResultAnalysis", () => {
  it("validates selected serial-test statistics at the result-query boundary", () => {
    const response = {
      kind: "serialTests",
      value: {
        dw: { d: 1.85 },
        q: { stat: 2.1, p_value: 0.3, lags: 5 },
        bg: { stat: 6.4, p_value: 0.04, lags: 3 },
      },
    };
    expect(parseResultAnalysis(response)).toEqual(response);
    expect(() => parseResultAnalysis({ ...response, value: { dw: 1.85 } })).toThrow(
      "Invalid result analysis: dw expected object",
    );
    expect(() =>
      parseResultAnalysis({
        ...response,
        value: { dw: { d: 2 }, bg: { stat: 1, p_value: 0.5 } },
      }),
    ).toThrow("Invalid result analysis: bg.lags missing required field");
  });
});
