/**
 * Info 报告种类（与 `features/application/results` 的 `ReportKind` 对齐）
 */

export const REPORT_PAYLOAD_KINDS = [
  "olsSummary",
  "binarySummary",
  "iv2slsSummary",
  "ivLimlSummary",
  "praisSummary",
  "varSummary",
  "varSoc",
  "panelSummary",
  "panelDid",
  "dfAdfSummary",
  "dfAdfSummaryList",
  "vecSummary",
  "vecRankSummary",
] as const;

export type ReportPayloadKind = (typeof REPORT_PAYLOAD_KINDS)[number];
