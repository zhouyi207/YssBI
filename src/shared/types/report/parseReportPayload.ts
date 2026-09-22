import type { ReportPayloadKind } from "./reportKinds";
import { parsedField, type ReportField, type ReportFieldResult } from "./fields";
import { parseDfAdfSummaryListResultData, parseDfAdfSummaryResultData } from "./parseDfadf";
import { panelDidResultDataField, panelSummaryField } from "./parsePanel";
import { binaryRegressionReportField, inlineRegressionReportField } from "./parseRegression";
import { linearRegressionReportField } from "./parseLinearRegression";
import { varSocResultDataField, varSummaryResultDataField } from "./parseVar";
import { vecRankResultDataField, vecSummaryResultDataField } from "./parseVec";

const reportFields = {
  linearRegressionSummary: linearRegressionReportField,
  binarySummary: binaryRegressionReportField,
  iv2slsSummary: inlineRegressionReportField,
  ivLimlSummary: inlineRegressionReportField,
  praisSummary: inlineRegressionReportField,
  varSummary: varSummaryResultDataField,
  varSoc: varSocResultDataField,
  panelSummary: panelSummaryField,
  panelDid: panelDidResultDataField,
  dfAdfSummary: parsedField(parseDfAdfSummaryResultData, "DF/ADF report"),
  dfAdfSummaryList: parsedField(parseDfAdfSummaryListResultData, "DF/ADF reports"),
  vecSummary: vecSummaryResultDataField,
  vecRankSummary: vecRankResultDataField,
} satisfies Record<ReportPayloadKind, ReportField<unknown>>;

export function parseReportPayloadResult(
  report: ReportPayloadKind,
  raw: unknown,
): ReportFieldResult<unknown> {
  if (!Object.prototype.hasOwnProperty.call(reportFields, report)) {
    return { ok: false, issue: { fieldPath: "$", reason: "expected a known report kind" } };
  }
  return reportFields[report].read(raw, "$");
}
