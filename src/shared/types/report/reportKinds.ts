/**
 * Info 报告种类（与 `features/application/results` 的 `ReportKind` 对齐）
 */

export type ReportPayloadKind =
  | "linearRegressionSummary"
  | "binarySummary"
  | "iv2slsSummary"
  | "ivLimlSummary"
  | "praisSummary"
  | "varSummary"
  | "varSoc"
  | "panelSummary"
  | "panelDid"
  | "dfAdfSummary"
  | "dfAdfSummaryList"
  | "vecSummary"
  | "vecRankSummary";
