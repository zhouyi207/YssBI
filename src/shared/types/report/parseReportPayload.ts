import { parseReportDisplay } from "@/shared/types/domain/structuredReportDisplay";
import type { ResultReportKind } from "@/shared/types/domain/result";
import type { ReportField, ReportFieldResult } from "./fields";
import { linearRegressionReportField } from "./parseLinearRegression";
import { isRecord } from "./guards";

const reportFields = {
  structured: {
    read: (raw: unknown, fieldPath: string) => {
      if (!isRecord(raw))
        return {
          ok: false as const,
          issue: { fieldPath, reason: "expected structured result data" },
        };
      try {
        parseReportDisplay(raw);
      } catch {
        return {
          ok: false as const,
          issue: { fieldPath: "report_display", reason: "invalid structured report bindings" },
        };
      }
      return { ok: true as const, value: raw };
    },
  },
  linearRegressionSummary: linearRegressionReportField,
} satisfies Record<ResultReportKind, ReportField<unknown>>;

export function parseReportPayloadResult(
  report: ResultReportKind,
  raw: unknown,
): ReportFieldResult<unknown> {
  if (!Object.prototype.hasOwnProperty.call(reportFields, report)) {
    return { ok: false, issue: { fieldPath: "$", reason: "expected a known report kind" } };
  }
  return reportFields[report].read(raw, "$");
}
