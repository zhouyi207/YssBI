import { expect, it, vi } from "vitest";
import type { ReportValidationDiagnostic } from "@/shared/types/report/reportValidation";
import { logger } from "@/utils/frontendLogger";
import { reportInvalidReport } from "./reportViewIssue";

it("reports only validated report identity without reading free diagnostic fields", () => {
  const report = vi.spyOn(logger.data, "error").mockImplementation(() => {});
  const privateField = vi.fn(() => "private report content");
  const diagnostic: ReportValidationDiagnostic = {
    resultId: "7",
    runId: "9",
    nodeId: "00000000-0000-4000-8000-000000000001",
    presentation: { kind: "report", report: "structured" },
    valueKind: "scalar",
    get outputPinId() {
      return privateField();
    },
    get fieldPath() {
      return privateField();
    },
    get reason() {
      return privateField();
    },
  };
  Object.defineProperty(diagnostic.presentation, "private", {
    enumerable: true,
    get: privateField,
  });
  try {
    reportInvalidReport(diagnostic);
    expect(privateField).not.toHaveBeenCalled();
    expect(report).toHaveBeenCalledExactlyOnceWith(
      "report_validation_failed resultId=7 runId=9 nodeId=00000000-0000-4000-8000-000000000001 report=structured valueKind=scalar",
      "ReportValidation",
    );
  } finally {
    report.mockRestore();
  }
});
