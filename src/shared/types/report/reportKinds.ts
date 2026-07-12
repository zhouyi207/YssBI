/**
 * Info 报告种类（与 `features/core/resultSource` 的 `ReportKind` 对齐）
 */

export const REPORT_PAYLOAD_KINDS = [
  'olsSummary',
  'binarySummary',
  'iv2slsSummary',
  'ivLimlSummary',
  'praisSummary',
  'varSummary',
  'varSoc',
  'panelSummary',
  'panelDid',
  'dfAdfSummary',
  'dfAdfSummaryList',
  'vecSummary',
  'vecRankSummary',
] as const;

export type ReportPayloadKind = (typeof REPORT_PAYLOAD_KINDS)[number];

const REGRESSION_REPORTS = new Set<ReportPayloadKind>([
  'olsSummary',
  'binarySummary',
  'iv2slsSummary',
  'ivLimlSummary',
  'praisSummary',
]);

export function isRegressionReportKind(kind: ReportPayloadKind): boolean {
  return REGRESSION_REPORTS.has(kind);
}

export function isReportPayloadKind(value: string): value is ReportPayloadKind {
  return (REPORT_PAYLOAD_KINDS as readonly string[]).includes(value);
}
