import {
  parseReportDisplay,
  type ReportDisplaySection,
} from "@/shared/types/domain/structuredReportDisplay";
import type { ResultReportKind } from "@/shared/types/domain/result";
import type { LinearRegressionReportData } from "@/shared/types/domain/resultReport";
import type { ReportFieldResult } from "./fields";
import { linearRegressionReportField } from "./parseLinearRegression";
import { isRecord } from "./guards";

export type ParsedReportPayload =
  | {
      readonly kind: "structured";
      readonly data: Record<string, unknown>;
      readonly sections: Readonly<Record<string, ReportDisplaySection>>;
    }
  | { readonly kind: "linearRegressionSummary"; readonly data: LinearRegressionReportData };

export function parseReportPayloadResult(
  report: ResultReportKind,
  raw: unknown,
): ReportFieldResult<ParsedReportPayload> {
  if (report === "structured") {
    if (!isRecord(raw))
      return {
        ok: false,
        issue: { fieldPath: "$", reason: "expected structured result data" },
      };
    try {
      return { ok: true, value: { kind: report, data: raw, sections: parseReportDisplay(raw) } };
    } catch {
      return {
        ok: false,
        issue: { fieldPath: "report_display", reason: "invalid structured report bindings" },
      };
    }
  }
  if (report === "linearRegressionSummary") {
    const parsed = linearRegressionReportField.read(raw, "$");
    return parsed.ok ? { ok: true, value: { kind: report, data: parsed.value } } : parsed;
  }
  return { ok: false, issue: { fieldPath: "$", reason: "expected a known report kind" } };
}
