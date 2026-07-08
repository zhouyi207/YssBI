/**
 * Info 报告 IPC 单点窄化：ReportKind → 已校验 payload
 */

import { isRegressionReportKind, type ReportPayloadKind } from './reportKinds';
import { parseDfAdfSummaryListResultData, parseDfAdfSummaryResultData } from './parseDfadf';
import { parsePanelDidResultData, parsePanelSummaryResult } from './parsePanel';
import { parseRegressionResultData } from './parseRegression';
import { parseVarSocResultData, parseVarSummaryResultData } from './parseVar';
import { parseVecRankResultData, parseVecSummaryResultData } from './parseVec';

/**
 * 在报告进入 InfoView 前窄化 JSON。
 * 返回 null 表示格式无效（漂移、缺字段、类型错误）。
 */
export function parseReportPayload(report: ReportPayloadKind, raw: unknown): unknown | null {
  if (raw === null || raw === undefined) return null;

  if (isRegressionReportKind(report)) {
    return parseRegressionResultData(raw);
  }
  switch (report) {
    case 'varSummary':
      return parseVarSummaryResultData(raw);
    case 'varSoc':
      return parseVarSocResultData(raw);
    case 'panelSummary':
      return parsePanelSummaryResult(raw);
    case 'panelDid':
      return parsePanelDidResultData(raw);
    case 'dfAdfSummary':
      return parseDfAdfSummaryResultData(raw);
    case 'dfAdfSummaryList':
      return parseDfAdfSummaryListResultData(raw);
    case 'vecSummary':
      return parseVecSummaryResultData(raw);
    case 'vecRankSummary':
      return parseVecRankResultData(raw);
    default:
      return null;
  }
}
