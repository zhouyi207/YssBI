// @vitest-environment happy-dom
import { resultSessionFixture } from "@/tests/helpers/resultFixture";

import { readFileSync } from "node:fs";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ResultDescriptor } from "@/shared/types/domain/result";
import { ReportView } from "./ReportView";
import { validateReportPayload } from "@/shared/types/report/reportValidation";

const { logError } = vi.hoisted(() => ({ logError: vi.fn() }));

vi.mock("@/utils/frontendLogger", () => ({
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

  it("logs safe report identity while displaying a malformed canonical OLS diagnostic", () => {
    act(() => {
      root.render(
        <ReportView
          descriptor={descriptor}
          validation={validateReportPayload(
            descriptor,
            "linearRegressionSummary",
            malformedLinearRegressionReport,
          )}
        />,
      );
    });

    expect(container.querySelector('[role="alert"]')?.textContent).toBe(
      "Unable to render linear regression report: coefficients.rowCount missing required field.",
    );
    expect(container.querySelector('[role="alert"]')).not.toBeNull();
    expect(logError).toHaveBeenCalledExactlyOnceWith(
      "report_validation_failed resultId=42 runId=7 nodeId=ols-node report=linearRegressionSummary valueKind=scalar",
      "ReportValidation",
    );
    expect(logError.mock.calls[0][0]).not.toMatch(
      /outputPinId|fieldPath|reason|coefficients\.rowCount|missing required field/,
    );
  });

  it("shows the exact missing OLS presentation field path without including it in logs", () => {
    act(() => {
      root.render(
        <ReportView
          descriptor={descriptor}
          validation={validateReportPayload(descriptor, "linearRegressionSummary", {
            ...malformedLinearRegressionReport,
            presentation: {
              ...reportFixture.presentation,
              summary: { ...reportFixture.presentation.summary, items: undefined },
            },
            coefficients: { ...malformedLinearRegressionReport.coefficients, rowCount: 1 },
          })}
        />,
      );
    });

    expect(container.querySelector('[role="alert"]')?.textContent).toBe(
      "Unable to render linear regression report: presentation.summary.items missing required field.",
    );
    expect(logError).toHaveBeenCalledExactlyOnceWith(
      "report_validation_failed resultId=42 runId=7 nodeId=ols-node report=linearRegressionSummary valueKind=scalar",
      "ReportValidation",
    );
    expect(logError.mock.calls[0][0]).not.toMatch(
      /outputPinId|fieldPath|reason|presentation\.summary\.items|missing required field/,
    );
  });
});
