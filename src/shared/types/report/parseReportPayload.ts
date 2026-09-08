import type { ReportPayloadKind } from "./reportKinds";
import { parsedField, type ReportField, type ReportFieldResult } from "./fields";
import { parseDfAdfSummaryListResultData, parseDfAdfSummaryResultData } from "./parseDfadf";
import { panelDidResultDataField, panelSummaryField } from "./parsePanel";
import {
  binaryRegressionReportField,
  canonicalOlsReportField,
  linearRegressionReportField,
} from "./parseRegression";
import { varSocResultDataField, varSummaryResultDataField } from "./parseVar";
import { vecRankResultDataField, vecSummaryResultDataField } from "./parseVec";

const reportFields = {
  olsSummary: canonicalOlsReportField,
  binarySummary: binaryRegressionReportField,
  iv2slsSummary: linearRegressionReportField,
  ivLimlSummary: linearRegressionReportField,
  praisSummary: linearRegressionReportField,
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
