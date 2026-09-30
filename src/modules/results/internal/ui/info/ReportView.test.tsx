// @vitest-environment happy-dom
import { resultSessionFixture } from "@/tests/helpers/resultFixture";

import { readFileSync } from "node:fs";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ResultDescriptor } from "@/shared/types/domain/result";
import { ReportView } from "./ReportView";

const { logError } = vi.hoisted(() => ({ logError: vi.fn() }));

vi.mock("@/features/application/observability/appLogger", () => ({
  logger: { data: { error: logError } },
}));

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

const descriptor: ResultDescriptor = {
  resultId: "42",

  executionSessionId: resultSessionFixture,
  provenance: {
    runId: "7",
    graphPath: "events/main.yssbi-event",
    nodeId: "ols-node",
    output: {
      graphPath: "events/main.yssbi-event",
      port: { kind: "declared", nodeId: "ols-node", portKey: "result" },
    },
    createdAtMs: "100",
  },
  presentation: { kind: "report", report: "linearRegressionSummary" },
  valueKind: "scalar",
  metadata: null,
  totalCount: 1,
  title: "Linear Regression Summary",
};

const reportFixture = JSON.parse(
  readFileSync("src/tests/fixtures/node-system-contracts/ols-summary-report.json", "utf8"),
);
const malformedLinearRegressionReport = {
  ...reportFixture,
  resultRef: { executionSessionId: resultSessionFixture, resultId: "42" },
  coefficients: { kind: "tableRef", part: "coefficients" },
};

describe("ReportView", () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    logError.mockClear();
    container = document.createElement("div");
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
  });

  it("logs an actionable diagnostic for a malformed canonical OLS report", () => {
    act(() => {
      root.render(
        <ReportView
          descriptor={descriptor}
          report="linearRegressionSummary"
          data={malformedLinearRegressionReport}
        />,
      );
    });

    expect(container.querySelector('[role="alert"]')?.textContent).toBe(
      "Unable to render linear regression report: coefficients.rowCount missing required field.",
    );
    expect(container.querySelector('[role="alert"]')).not.toBeNull();
    expect(logError).toHaveBeenCalledTimes(1);
    expect(JSON.parse(logError.mock.calls[0][0])).toEqual({
      resultId: "42",
      runId: "7",
      nodeId: "ols-node",
      outputPinId: "result",
      presentation: { kind: "report", report: "linearRegressionSummary" },
      valueKind: "scalar",
      fieldPath: "coefficients.rowCount",
      reason: "missing required field",
    });
    expect(logError).toHaveBeenCalledWith(expect.any(String), "ReportValidation");
  });

  it("reports the exact missing OLS presentation field path", () => {
    act(() => {
      root.render(
        <ReportView
          descriptor={descriptor}
          report="linearRegressionSummary"
          data={{
            ...malformedLinearRegressionReport,
            presentation: {
              ...reportFixture.presentation,
              summary: { ...reportFixture.presentation.summary, items: undefined },
            },
            coefficients: { ...malformedLinearRegressionReport.coefficients, rowCount: 1 },
          }}
        />,
      );
    });

    expect(container.querySelector('[role="alert"]')?.textContent).toBe(
      "Unable to render linear regression report: presentation.summary.items missing required field.",
    );
    expect(logError).toHaveBeenCalledTimes(1);
    expect(JSON.parse(logError.mock.calls[0][0])).toMatchObject({
      resultId: "42",
      runId: "7",
      nodeId: "ols-node",
      fieldPath: "presentation.summary.items",
      reason: "missing required field",
    });
  });
});
